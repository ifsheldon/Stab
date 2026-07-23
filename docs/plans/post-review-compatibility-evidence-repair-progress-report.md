# Post-Review Compatibility And Evidence Repair Progress Report

## Status

Source remediation and dirty-tree diagnostic verification are complete as of 2026-07-23. R0 through R5 are implemented. R6 is blocked by its deliberate clean-commit prerequisite, not by an implementation defect. The dirty-tree portion of R7 is complete; final evidence, repeat audits, and clean-worktree acceptance remain downstream of R6.

This report follows [post-review-compatibility-evidence-repair.md](post-review-compatibility-evidence-repair.md) and [GOAL.md](GOAL.md). It does not promote dirty-tree output as qualification evidence.

## Source State

- Compatibility target: Stim v1.16.0 at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- Current branch: `main`.
- Current worktree: modified; therefore not eligible for promotable evidence.
- Correctness inventory digest: `592934174f3cf248553d3df67078ec00563e48acfd4c5ddf15cef44fd9b49fd0`.
- Performance inventory digest: `33b796a2eda59429fcccc43a3db8dc715608e5dffabd9cfe1b756c4d40529358`.
- Public API inventory: 2,065 items.
- Correctness evidence parents: 1,757 total, 593 implemented, 17 evidence-close, and 1,147 planned.
- Result-format evidence parents: 43.
- Performance thresholds: unchanged; every promotable comparative gate remains `1.25x`.
- Review-rejected history: the DEM chain at `80fb5405fb077c694a8a8a18e64a3a5831e20a5e` remains historical `raw-work-v1` evidence and is not current evidence.

## Milestone Audit

### Findings

No unresolved implementation or milestone-specification finding has been confirmed in R0 through R5.

R6 is blocked by an explicit prerequisite in the plan: promotable evidence may be generated only after the user authorizes focused commits and the source revision is clean and unchanged. Running the formal suite now would violate the evidence contract.

The first workspace-test pass found that `m2d` opened a requested observable-output path before rejecting its unsupported `ptb64` format. The existing regression test required the path to remain absent. Static output-format validation now runs before the I/O preflight, and the focused test passes. This finding was fixed before continuing the audit.

The second workspace-test pass found a stale source-inventory assertion: it counted 38 `cq2-result-*` qualification-plan parents after four new independently selectable owners increased the generated total from 36 to 40. The four source cases and their ownership were inspected, the assertion was corrected to the generated total, and the inventory test was rerun.

The otherwise passing broad run also exposed an untracked empty `obs.01` created by the `sample_dem` conflicting-route test. Static route and `ptb64` validation for `sample`, `detect`, and `sample_dem` had still occurred after non-truncating output creation. These commands now validate those argument-only contracts before constructing the I/O plan, and their regression tests require requested output paths to remain absent.

### Milestone Status

Status: Blocked

Rationale: the implementation milestones are satisfied on the dirty worktree, but the complete remediation cannot be accepted until R6 produces fresh correctness and `raw-work-v2` performance evidence from one clean committed revision. This is an intentional provenance gate, not permission to weaken or bypass R6.

### Completion Matrix

| Requirement | Status | Evidence | Notes |
| --- | --- | --- | --- |
| R0 freeze claims and scope | Satisfied | `README.md`; `docs/stab-feature-checklist.md`; `docs/plans/GOAL.md`; `docs/plans/pq2-dem-parse-print-qualification-progress-report.md` | Public wording is narrowed and the old DEM chain is explicitly historical and review-rejected. |
| R1 prevent file-alias data loss | Satisfied | `crates/stab-cli/src/io_plan.rs`; `crates/stab-cli/src/tests/path_alias.rs` | Inputs and non-truncating outputs retain descriptor identities; every active input/output and output/output alias is rejected before regular-file truncation. |
| R2 byte-exact grammars and typed DETS | Satisfied | `crates/stab-core/src/result_text.rs`; `crates/stab-core/src/result_formats/dets.rs`; `crates/stab-core/tests/result_text_compat.rs`; `crates/stab-cli/src/tests/result_text_compat.rs` | One lexer owns grammar; DETS namespaces and duplicate semantics are consumer-specific and layout-aware. |
| R3 oracle and qualification ownership | Satisfied | `oracle/result-format-corpus.json`; `ops/oracle/src/result_format_corpus.rs`; `oracle/qualification-manifest.json` | The checked corpus contains 62 pinned cases and corrected behaviors have independently selectable owners. |
| R4 shared bounded process supervisor | Satisfied | `ops/bench/src/process.rs`; `ops/bench/src/process/tests.rs`; `target/benchmarks/post-review-process-baseline-20260723/baseline.json` | Baseline and qualification callers use one supervisor; the adversarial process probe, benchmark smoke, pinned Stim build, and a real CLI baseline pass; the legacy runner and direct `wait-timeout` dependency are removed. |
| R5 corrected timer boundary | Satisfied | `ops/bench/src/qualification/runtime/group.rs`; `ops/bench/src/qualification/runtime/worker.rs`; `benchmarks/stim_adapter/main.cc` | Both workers identify `raw-work-v2`; schema and adversarial checks reject stale or mismatched identities. Formal worker reproducibility belongs to R6. |
| R6 rebuild correctness and performance evidence | Blocked | Clean-commit prerequisite in the active plan | No formal artifact has been generated or promoted from the dirty worktree. No artifact path has been reused. |
| R7 documentation, audit, review, and closure | Partially satisfied | This report and synchronized public documentation | Dirty-tree documentation, audit, review, and broad verification are complete; revision-bound evidence, repeat audits, and clean-worktree acceptance depend on R6. |

### Specification Gaps

The implementation clarified two genuine details and recorded them in [milestone-spec-gaps.md](milestone-spec-gaps.md):

- active roles, non-regular output activation, and race-safe failed-preflight cleanup;
- separation between byte-grammar acceptance, consumer-specific duplicate semantics, and successful-prefix behavior.

These clarifications tighten the plan and do not excuse an implementation defect or weaken compatibility.

## Full Code Review

The manual review covers:

- descriptor-based CLI path identity and truncation ordering;
- pinned-Stim `01`, HITS, and DETS grammar and duplicate semantics;
- public typed DETS ownership and reusable visitor buffers;
- corpus independence from round-trip-only evidence;
- bounded subprocess I/O, timeout, cancellation, descendant cleanup, diagnostics, affinity, RSS, and file limits;
- `raw-work-v2` ordering and schema propagation;
- generated inventory and public-document alignment;
- operational command ownership and source-file size.

No unresolved confirmed P0 through P3 finding is currently recorded. No non-generated Rust source exceeds 1,200 lines. The touched `result_formats.rs` and shared process supervisor remain watch-list files above 900 lines, but their new grammar, DETS, and test responsibilities have separate modules and do not currently reveal mixed ownership.

## Verification

### Passed

- `just qualification::correctness-check`
  - digest `592934174f3cf248553d3df67078ec00563e48acfd4c5ddf15cef44fd9b49fd0`;
  - 2,065 public API items;
  - 1,757 evidence parents;
  - 593 implemented, 17 evidence-close, and 1,147 planned;
  - 43 result-format evidence parents.
- `just qualification::correctness-regenerate --check`.
- `just bench::qualification-regenerate --check`.
- `just bench::qualification-check`
  - digest `33b796a2eda59429fcccc43a3db8dc715608e5dffabd9cfe1b756c4d40529358`;
  - all `1.25x` thresholds unchanged.
- `cargo fmt --all --check`.
- `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo test --workspace --quiet`.
- `just oracle::version`.
- `just oracle::result-formats --check`
  - 62 cases;
  - 20 accepted and 42 rejected;
  - pinned Stim and Stab both checked.
- `just oracle::run --implemented-only`.
- `just oracle::matrix --check`
  - 313 rows.
- `just bench::smoke`
  - 161 planned rows.
- `just bench::qualification-probe --group pq1-process-contract-smoke`.
- `just bench::baseline --only m7-convert-01-to-b8 --out target/benchmarks/post-review-process-baseline-20260723`
  - one measured real `stim convert` CLI row;
  - the destination was absent before the diagnostic run.
- `just maintenance::pre-commit`
  - passed as a staged-aware no-op because no files are staged.
- `git diff --check`.
- no `stab-oracle qualification correctness` or `stab-bench qualification` process remains.
- swap was not changed during dirty-tree diagnostics; `/swap.img` remains active.
- Focused core grammar, corpus, DETS, visitor, and result-format tests.
- Focused CLI path-alias, convert, replay, `m2d`, and result-format corpus tests.
- Shared process-supervisor tests, including output-before-input deadlock, floods, timeout, cancellation, descendant-held pipes, invalid UTF-8, discard, and bounded diagnostics.
- Qualification timing-boundary, protocol, report, and adversarial schema tests.
- Focused `stab-bench` Clippy with warnings denied.

### Intentionally Not Run

- formal correctness PR, full, and soak evidence;
- fresh primary baseline, beta gate, timing regression, and memory regression;
- formal worker reproducibility and DEM adapter probes;
- twelve DEM reports and regressions;
- four architecture rollups and two completion receipts;
- accepted-maximum memory probes;
- swap changes.

These operations are R6 outputs and require an explicitly authorized clean source commit. The absence of those artifacts is not converted into a waiver or a current performance claim.

## R6 Handoff

After the user authorizes focused commits:

1. Commit source behavior, tests and corpus, process/timing contracts, generated inventories, and documentation in reviewable focused commits.
2. Verify `local_modifications=false` and record the unchanged source revision.
3. Allocate fresh artifact paths that have never held failed or historical output.
4. Record the current swap configuration, disable swap only immediately before formal timing, and restore the exact prior configuration on every exit path.
5. Execute every R6 correctness, legacy benchmark, DEM report, replay, regression, rollup, completion, and memory command.
6. Update this report with revision-bound paths and outcomes.
7. Repeat milestone audit, full code review, final verification, process cleanup, and swap verification.
