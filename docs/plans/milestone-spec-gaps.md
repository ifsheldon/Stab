# Milestone Under-Specification Log

This log records milestone loopholes, ambiguous acceptance criteria, and under-specified scope discovered during milestone implementation or milestone audit.
Use this file for specification gaps only.
Implementation defects, missing tests, benchmark failures, documentation omissions, and code-review findings should be fixed in the milestone work unless a separate follow-up is explicitly accepted.

## Entry Format

```text
## YYYY-MM-DD - Mx: Milestone Title

Status: Open | Resolved | Superseded
Revealed by: implementation, test, benchmark, audit, or review evidence
Current text: the milestone wording that was too weak or ambiguous
Gap: what the milestone failed to specify
Proposed amendment: concrete replacement text or additional done criterion
Resolution: link or note for the plan update that resolved the gap
```

## Resolved Entries

## 2026-06-27 - M2: Comparator Implementation Ownership

Status: Resolved
Revealed by: milestone audit of the M2 oracle corpus.
Current text: M2 said to define structural and statistical comparators, while later milestones own the first runnable uses of many semantic and statistical comparator families.
Gap: the plan did not say whether M2 must implement every comparator executable or only define comparator contracts and fixture metadata before implementation milestones begin.
Proposed amendment: state that M2 defines comparator contracts and manifest metadata, while the owning M4 through M11 milestones must implement runnable structural or statistical comparator code before marking matching rows `implemented`.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now makes comparator implementation ownership explicit in the M2 task list.

## Open Entries

## 2026-06-27 - M2: Manifest-Only Subcase Granularity

Status: Open
Revealed by: milestone audit of the M2 manifest coverage rows.
Current text: M2 and the test-porting plan allow red or manifest-only oracle cases for all P0 and P1 files needed by M4 through M11.
Gap: file-level manifest-only rows can satisfy coverage without identifying the upstream subcases, fixture families, malformed-input cases, or extraction criteria that future implementation milestones must port.
Proposed amendment: require manifest-only rows to name planned subcase groups or extraction criteria for each upstream test file before the owning implementation milestone starts.
Resolution: pending plan update.

## 2026-06-26 - M1/M4/M7: CLI Convert Ordering

Status: Open
Revealed by: milestone audit of the M1 compatibility matrix and `just oracle::matrix --milestone M4`.
Current text: M1 says planned CLI surfaces are covered in implementation order as `gen`, `convert`, `sample`, `detect`, `m2d`, `analyze_errors`, and `sample_dem`; M4 links `src/stim/cmd/command_convert.test.cc` for parse/canonical-print behavior; M7 tasks say to implement both `stim gen` and `stim convert`.
Gap: the plan does not clearly say whether M4 implements a public `stim convert` subset, only internal parse-print oracle fixtures, or test metadata that M7 later turns into CLI compatibility.
Proposed amendment: state that M4 owns the `.stim` parser/printer library contract and may use `command_convert.test.cc` only as oracle evidence for parse/canonical-print semantics, while M7 owns public `stim convert` CLI compatibility unless the plan explicitly promotes a minimal M4 CLI subset.
Resolution: pending plan update.

## 2026-06-26 - M0: Upstream Smoke References Overreach

Status: Open
Revealed by: milestone audit of the M0 oracle lab implementation.
Current text: M0 links `src/stim.test.cc`, `src/stim/main_namespaced.test.cc`, and `src/stim_included_twice.test.cc` as C++ smoke references.
Gap: those upstream files include behavior from later milestones, including circuit parsing, gate metadata, analyzer behavior, and richer CLI mode handling, so treating the full files as M0 requirements would pull M4, M6, and M10 work into the foundation milestone.
Proposed amendment: clarify that M0 extracts only oracle-process smoke checks from these files, specifically help-command health, main binary namespacing health, and one tiny deterministic circuit case; all parser, gate table, analyzer, and broader CLI behavior stays with later milestones.
Resolution: pending plan update.

## 2026-06-26 - M0: Oracle Tiny Sample Shim Boundary

Status: Open
Revealed by: milestone audit and full-code-review of the M0 `stab-cli sample` smoke shim.
Current text: M0 requires `just oracle::run --case smoke/tiny-circuit`, while the CLI compatibility order defers real `sample` support to M8.
Gap: the plan does not say whether a minimal M0 sample command counts as CLI compatibility or is only an oracle fixture target.
Proposed amendment: state that any M0 sample path is an oracle-only smoke shim and does not count as implemented `stim sample` compatibility; M8 remains responsible for the public `sample` command contract.
Resolution: pending plan update.

## 2026-06-26 - M0: Benchmark Smoke Before Benchmark Harness

Status: Open
Revealed by: milestone audit and full-code-review of `just bench::smoke`.
Current text: M0 requires CI benchmark smoke tests, while M3 owns the benchmark package, baseline measurements, benchmark matrix, and performance contracts.
Gap: before M3, benchmark smoke can only prove workspace wiring unless the plan requires an explicit placeholder benchmark target.
Proposed amendment: clarify whether M0 benchmark smoke is compile-only workspace smoke or require a tiny explicit benchmark target that is intentionally replaced by the M3 benchmark harness.
Resolution: pending plan update.
