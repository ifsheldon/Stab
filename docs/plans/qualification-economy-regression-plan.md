# Qualification Simplification And Regression Plan

## Status

Active qualification contract as of 2026-07-23.

This plan supersedes the R6 evidence-production procedure in [post-review-compatibility-evidence-repair.md](post-review-compatibility-evidence-repair.md) before formal repaired-contract evidence began. The R0 through R5 implementation remains accepted source work. Historical and failed artifacts retain their original schema, source revision, and status.

Implementation checkpoint: Q0 through Q7 are implemented in source and undergoing final contract validation, milestone audit, and code review. Q8 formal timing and completion evidence has not started.

## Objective

Preserve Stab's compatibility, filesystem-safety, process-supervision, and benchmark-science guarantees while reducing low-value tests, speculative benchmark obligations, duplicated corpus code, and evidence ceremony. Separate Stim parity from Stab self-regression, curate a finite release matrix, add representative DEM workloads, enforce source-owned qualification contracts in CI, and produce formal evidence once under the frozen replacement contracts.

No Stab product API or CLI behavior changes are part of this plan. Stim v1.16.0 remains the compatibility target, the parity gate remains `1.25x`, and `raw-work-v2` remains the current timing boundary.

## Contract Changes

- Add a private `stab-compat-corpus` workspace crate for result-format corpus schema, validation, decoding, selection, and canonical expected values.
- Add `future-candidate` as a performance disposition for real but unselected workload candidates.
- Add `family_id` and `size_class` to runtime scales and rename baseline eligibility to parity eligibility.
- Rename `qualification-baseline.json` to `qualification-parity-policy.json`.
- Add a source-owned self-regression policy and architecture-specific accepted baselines.
- Replace the active per-group completion workflow with one architecture/revision completion manifest while retaining historical receipt readers.
- Keep worker protocol schema 5 and `raw-work-v2`. Bump the runtime-group, performance-inventory, preflight, report, rollup, and completion schemas when their serialized contracts change.

## Q0: Freeze The Replacement Program

Log this plan, make `GOAL.md` the short execution contract, mark the old R6 procedure superseded, preserve historical artifacts, and reduce active documentation to the goal, two normative qualification contracts, and a generated status dashboard.

Acceptance requires every current document to state that repaired-contract formal evidence has not started and that this plan owns the next evidence run.

## Q1: Improve Test Economy

Move duplicated result-format corpus plumbing into `stab-compat-corpus` while retaining semantic assertions in core, CLI, and oracle owners. Remove standalone assertions for ordinary derived traits, trivial accessors, type-name `Debug` output, and marker inequality when stronger behavioral evidence already owns the API. Reassign `SampleFormat` ownership to exact writer-byte coverage.

Replace DETS visitor pointer-identity assertions with allocation instrumentation proving width-bounded allocation, no growth with record count, immediate cancellation, and bounded retained capacity. Preserve clone tests that prove state independence or mutation isolation.

Acceptance requires one corpus implementation, all 62 pinned cases to retain coverage, and no removed test to have owned meaningful compatibility behavior.

## Q2: Curate The Performance Matrix

Keep the complete public API inventory as a coverage map instead of turning each behavioral item into a planned benchmark. Remove synthetic planned groups generated from API and checklist ownership. Classify unselected workloads as `covered-by-parent`, `not-performance-relevant`, or `future-candidate`.

The initial release matrix contains 19 runtime groups representing 23 gated workload families: 17 existing non-DEM groups plus DEM parse and print across three fixture families. Enforce caps of 40 release groups and 60 diagnostic groups. Retain a legacy benchmark row only until accepted qualification evidence replaces it.

Acceptance requires every active release group to have an executable contract, correctness prerequisite, parity disposition, and meaningful workload.

## Q3: Separate Parity From Regression

Keep Stim parity at paired median and confidence-interval upper bound no greater than `1.25`. Add architecture-specific Stab self-regression baselines with a default 15% tolerance. Source-owned exceptions must be committed before measurement, justified, and no greater than 25%.

Gate both current median against accepted median and current upper bound against accepted upper bound. Key baselines by workload, family, scale, measurement, host and CPU identity, target, toolchain, Stim build, timing boundary, and a semantic workload-contract digest. Treat a missing or mismatched baseline as `unseeded`, never as passing.

Generate a candidate baseline only from accepted full and soak rollups with identical identities, recording the worse median and upper bound. The first AArch64 run seeds a baseline but cannot retroactively claim a self-regression pass.

Acceptance requires parity, self-regression, environment validity, and memory/scaling outcomes to be reported independently.

## Q4: Reduce Global Preflight Ceremony

Replace the fixed 228-receipt reproducibility matrix with one accepted small receipt per active runtime group and implementation plus three shared rejection classes per implementation. The initial contract contains 46 ordered receipts and derives accepted probes from active runtime groups. Enforce a hard cap of 128 receipts.

Keep group-specific odd/even, accepted-maximum, over-cap, malformed, and semantic-overflow cases in focused unit tests and adapter probes.

Acceptance requires deterministic ordering and failure on missing, extra, duplicate, stale, wrong-implementation, or shared-rejection evidence.

## Q5: Add Representative DEM Families

Keep two runtime groups, DEM parse and DEM print, with nine scales each. Each operation covers small, medium, and large variants of:

- `flat-errors`: flat error-heavy throughput with varied probabilities and target combinations.
- `coordinate-sparse`: tags, coordinates, shifts, and sparse high detector and observable identifiers.
- `folded-repeats`: nested compact repeats with large repeat counts that remain folded.

Use deterministic independent Rust and C++ generators, exact input digests, and exact semantic-output comparison with only the documented terminal-newline normalization. Use 64, 4,096, and 65,536 compact work items. Accepted maxima are 524,288 for flat and coordinate families and 262,144 for folded repeats.

Acceptance requires six operation/family workloads with correctness prerequisites, executable contracts, parity rules, profiler notes, cross-worker fixture checks, boundary tests, and family-local monotonic scales.

## Q6: Simplify Evidence Publication

Preserve descriptor-safe opening, bounded subprocesses, source validation, immutable output paths, and atomic publication. Keep raw reports and full/soak group rollups.

Replace per-group step transcripts and completion trees with one architecture/revision manifest that binds repository, Stim, toolchain, host, workers, inventories, policies, correctness, reports, rollups, memory evidence, parity, and regression outcomes. Rollup replay validates each source report and parity result. One offline completion replay reconstructs the summary.

Retain a read-only historical completion parser and remove the old active `CompletionStep` producer.

Acceptance reduces the DEM evidence program to 36 raw reports, four rollups, one completion manifest, and one offline replay.

## Q7: Add CI And Generated Status

Add a non-timing CI job for correctness inventory checks, performance inventory checks, generated status, and the live 62-case result-format oracle. Rename the shared-host scheduled workflow and artifacts to `M12 Diagnostic Performance Trend`.

Generate `docs/qualification-status.md` from checked inventories, runtime contracts, parity policy, regression baselines, and the current completion checkpoint. README and the feature checklist link to this dashboard instead of duplicating volatile counts.

Acceptance requires checked-in status drift to fail CI and shared-host timing to be clearly non-authoritative.

## Q8: Freeze And Produce Formal Evidence

Commit Q0 through Q7 in focused changes, run milestone audit and full code review, fix confirmed findings, regenerate contracts from a clean revision, and then run reopened correctness PR, full, and soak tiers plus the live result-format corpus.

Run legacy primary timing and memory checks as diagnostics. On the controlled AArch64 host, disable swap immediately before formal timing and restore the exact prior configuration on every exit. Produce 36 DEM reports, four rollups, one completion manifest, one offline replay, and accepted-maximum memory evidence using unique paths.

Generate the first AArch64 self-regression baseline candidate in a separate reviewed commit. Keep x86-64 unseeded until controlled native evidence exists. Finish with milestone audit, full code review, standard workspace checks, oracle checks, qualification checks, benchmark smoke, pre-commit, restored host state, and a clean worktree.

## Assumptions

- Strict compatibility tests, path-alias protection, byte-exact grammars, descriptor-safe access, bounded process supervision, paired timing, and `raw-work-v2` remain unchanged.
- The new completion manifest replaces the old active ceremony instead of adding another layer.
- The release matrix grows only from demonstrated performance risk.
- Regression exceptions are committed before evidence and capped at 25%.
- Historical evidence remains readable but never becomes current through schema migration.
- Intentionally deferred Stim and ecosystem surfaces remain outside this program.
