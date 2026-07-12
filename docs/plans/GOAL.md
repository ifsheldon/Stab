# Goal: Finalize The Non-Deferred Blocker Rollup

## Mission

Finish PFM-B6 in `docs/plans/non-deferred-partial-feature-milestones.md`.
PFM-B1 through PFM-B5 are implemented for the selected Rust and CLI scope; this goal now owns only final review remediation, cross-document synchronization, verification, and a conservative completion record.
Do not add new product scope under this goal.

Read `docs/plans/lessons-learned.md` before changing acceptance claims.
A green test is not sufficient when provenance, benchmark classification, resource behavior, or documentation still overstates what was proved.

## Current Checkpoint

- PFM-B0 froze the schema-version-2 blocker ledger.
- PFM-B1 closed nineteen reverse-flow and QEC-transform cases.
- PFM-B2 closed one analyzer sweep evidence case and 37 independently selectable gate-by-surface cases covering all nineteen semantic families.
- PFM-B3 closed seven shared folded-DEM traversal contracts.
- PFM-B4 closed two detecting-region cases, fourteen missing-detector cases, and thirty-three flow cases.
- PFM-B5 closed fifty-two analyzer, search, SAT/WCNF, sparse-tracker, and matched-error cases.
- `just oracle::blockers --check-selectors` validates 165 cases with no planned row and no shared selector.
- PFM-B2 review remediation is committed in `f1f6e42`, oracle hardening in `6bdff8b`, and the first split benchmark runner in `6576273`.
- Final-review remediation removes the sweep-reference record copy in `2f46c33`, unifies canonical surface and statistical boundaries in `25f352b`, requires exact upstream gate markers in `8ab85e4`, and exposes detector-frame timing without heterogeneous medians in `fb47b03`.
- The clean reports from `6576273` are superseded because the final-review benchmark and hot-path fixes changed the measured contract. Fresh clean timing and allocation reports are required before completion.

There is no remaining implementation blocker in the finite ledger.
The only remaining blockers are confirmed findings from the final GPT-5.6/max re-review, documentation disagreement, or a failing final verification command.

## Active Sources Of Truth

- Execution and acceptance: `docs/plans/non-deferred-partial-feature-milestones.md`.
- Executable inventory: `docs/plans/blocker-closure-ledger.json`.
- Planning resolutions: `docs/plans/milestone-spec-gaps.md`.
- Final gate evidence: `docs/plans/pfm-b2-gate-surface-progress-report.md`.
- Final rollup: `docs/plans/pfm8-rollup-evidence-report.md`.
- User-facing status: `docs/stab-feature-checklist.md`.
- Child-surface inventory: `docs/plans/partial-feature-inventory.md`.
- Historical roadmap and test hierarchy: `docs/plans/rust-stim-drop-in-rewrite.md` and `docs/plans/stim-test-porting-plan.md`.
- Frozen compatibility oracle: Stim v1.16.0 in `vendor/stim`.
- Lessons: `docs/plans/lessons-learned.md`.

If these sources disagree, fix the disagreement before completion.

## Required Work

1. Complete the final GPT-5.6/max full-code-review over core, CLI, oracle, benchmark, and documentation changes.
2. Fix every confirmed correctness, compatibility, hostile-input, performance, resource, architecture, evidence, and documentation finding.
3. Log only genuine newly revealed under-specification in `docs/plans/milestone-spec-gaps.md`; do not use a new entry to avoid a decision already made by PFM-B0 through PFM-B5.
4. Finish PFM-B6 synchronization across the checklist, partial inventory, roadmap, test-porting plan, PFM8 rollup, PFM-B2 report, historical progress reports, README, oracle metadata, and benchmark metadata where affected.
5. Run the final verification commands from the resulting worktree.
6. Commit the final review fixes and documentation in focused commits.

## Status Rules

Use `Done for selected Rust API scope` or `Done for selected Rust/CLI scope` when every non-deferred child in that selected surface is complete.
Keep a literal full-product rollup `Partial` only when a named deferred product prevents an honest full-Stim claim.
Do not describe deferred Python, JS/WASM, diagrams, ecosystem packages, public simulator products, full ErrorMatcher provenance, exact random streams, or deprecated behavior as active implementation blockers.
Future behavior outside the 165-case ledger requires a new exact plan with named subcases, comparators, tests, resource contracts, oracle disposition, and benchmark disposition.

## Evidence Rules

- Every completion claim must point to an independently selectable test, an evidence-close contract, or a source-owned report.
- Exact GTest provenance must identify a complete macro name and a matching gate marker inside that test body when the subcase names a gate.
- Statistical plans must remain bijective with the core catalog and use bounded exact-tail evaluation only after the frozen digest matches.
- Probabilistic tests must retain source-owned shots, seeds, bucket probabilities, tolerance rules, and familywise false-positive budgets.
- Resource claims must use direct allocation regressions or allocation-tracked reports, not infer constant memory from semantic success.
- Report-only and contract-only rows must not imply a Stim ratio, beta-gate pass, or primary threshold.
- Clean benchmark evidence must record the committed Stab revision and `local_modifications=false`.

## PFM-B2 Evidence To Regenerate

The final representative timing row must report seven independent measurements:

| Surface | Required evidence |
| --- | --- |
| Sampler execution | Independent normalized circuits/s |
| Reference sampling | Independent normalized circuits/s |
| Converter compilation | Independent normalized circuits/s |
| Ordinary detection sampling | Independent normalized circuits/s over the non-frame corpus |
| Forced detector-frame sampling | Independent normalized circuits/s over the full representative corpus |
| Error analysis | Independent normalized circuits/s |
| Flow generation | Independent normalized circuits/s |

The Markdown report must leave the row-level Stab median empty and render every normalized submeasurement explicitly.
The gate and sweep allocation reports must be regenerated from the same clean revision.
These rows remain report-only and have no faithful pinned-Stim timing ratio.

## Final Verification

Run after all review and documentation fixes:

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings
cargo test --workspace --quiet
cargo test -p stab-core --features ops-contracts warmed_fixed_tableau_gate_execution_does_not_allocate_per_dispatch --quiet
cargo test -p stab-core --features ops-contracts streamed_sweep_conversion_adds_no_per_shot_scratch_allocations --quiet
just oracle::blockers --check-selectors
just oracle::run --milestone PF3
just oracle::run --milestone M8
just oracle::run --implemented-only
just bench::smoke
just maintenance::pre-commit
```

A fresh primary benchmark run is required only if final remediation changes a primary runner, primary threshold, or shared primary hot path.
The current PFM-B2 changes are report-only and do not alter the 1.25x primary gate.

## Completion Criteria

This goal is complete only when:

- Final GPT-5.6/max review has no unresolved confirmed finding.
- All blocker-program entries in `docs/plans/milestone-spec-gaps.md` are resolved.
- The source ledger remains at 165 cases with zero planned rows and independent selectors.
- PFM-B1 through PFM-B5 progress reports and the PFM8 rollup agree on completion.
- The checklist contains no non-deferred child row left `Partial` because of stale wording.
- Literal product-level partial statuses identify the exact deferred products that prevent full parity.
- Clean PFM-B2 timing and allocation evidence remains reproducible and honestly report-only.
- Every final verification command passes.
- Focused commits contain the final review remediation and documentation closure.

## Explicit Deferrals

Python bindings and Python object shape, JS/WASM, diagrams, `repl`, QASM, Quirk, Crumble, ecosystem packages, GPU, exact random-stream parity, public graph or vector simulator products, C++ header compatibility, full ErrorMatcher provenance, `explain_errors`, deprecated `--detector_hypergraph`, and behavior outside the finite selected ledger remain future work.
They do not block this goal.
