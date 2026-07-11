# PFM8 Rollup Evidence Report

Date: 2026-07-10

Status: In progress, not a final PFM8 completion report.

## Scope

This report records the current PFM8 evidence state after the PFM0 broad active-wording reconciliation committed as `1f80348 docs(plans): lock broad partial-feature scope`, the clean PFM0 evidence refresh committed as `8f80612 docs(plans): refresh PFM0 evidence lock`, the selected PFM2 MPAD duplicate observable-id record parity slice committed as `3e30552 fix(core): merge duplicate MPAD observable records`, the selected PFM5 observable-neutral final-repeat missing-detector slice committed as `525d734 fix(core): fold observable-neutral missing-detector repeats`, and the selected PFM2 pinned feedback public-method evidence repair committed as `0cf2d3e test(core): pin feedback transform evidence`.
It also incorporates the completed PFM-B3 shared folded DEM traversal implementation committed as `4a984c2 feat(core): add shared folded DEM traversal`.
It covers the rollup layer only: `Rust core library equivalent for core Stim semantics`, `.stim`/`.dem`/result-format compatibility, `Full semantic execution of every legal circuit operation`, `Highest-priority remaining feature gaps`, and the selected CLI binary status.
It does not add production behavior, promote a new active feature subcase, or claim full Stim parity.

## Source-Of-Truth Inputs

- `docs/plans/GOAL.md` says the goal is complete only when every non-deferred partial row has implemented evidence or a named deferred subcase, documentation agrees with behavior, and milestone-audit plus full-code-review findings are fixed or logged as true under-specification.
- `docs/plans/non-deferred-partial-feature-milestones.md` says PFM8 may update rollup rows only after every active child row is implemented or explicitly deferred with a named reason.
- `docs/plans/partial-feature-inventory.md` maps current partial rows to active PFM owners, implemented child evidence, deferred-only exclusions, and manifest-only extraction contracts.
- `docs/plans/milestone-spec-gaps.md` records the original broad active wording, the planning loophole it exposed, and the PFM-B milestone now selected to resolve it.
- `docs/plans/blocker-closure-ledger.json` assigns every remaining blocker to a finite PFM-B subcase with comparator, evidence state, test selector, oracle disposition, benchmark disposition, and resource contract.
- `docs/stab-feature-checklist.md` remains the user-facing feature status document and still marks rollup or broad scoped rows as `Partial` where broader Stim parity is not proven.

## Current Evidence Snapshot

Implemented oracle evidence is healthy for the current selected Rust and CLI surface.
Earlier PFM8 snapshots recorded local-modification evidence while the report, PF3 `MPAD`, noisy `MPAD(p)`, and deterministic `MPP` evidence rows were still being synchronized.
After the selected PFM2 MPAD duplicate observable-id record parity slice was committed, `just oracle::run --implemented-only` passed on 2026-07-08 from clean committed `HEAD=3e305525bc9c` with `local_modifications=false`, including `pf2-inverse-qec-mpad-rust`.
After the PFM0 evidence-lock cleanup, selected PFM5 observable-neutral final-repeat missing-detector slice, and selected PFM2 pinned feedback public-method evidence repair landed, the current refresh passed again on 2026-07-08 from clean committed `HEAD=0cf2d3eee423` with `local_modifications=false`, including `pf5-missing-detectors-observable-neutral-final-repeat-rust` and `pf2-feedback-inline-pinned-upstream-rust`.
On 2026-07-10, `just oracle::run --implemented-only`, `just oracle::blockers --check-selectors`, and `just bench::smoke` passed from clean PFM-B3 implementation `HEAD=4a984c26b39f6236fde5e3ff10cf0b42e8b155a2`.

Metadata evidence is healthy for the current manifests.
The current PFM8 verification pass reran oracle, matrix, and benchmark metadata checks after the latest committed PFM0, PFM2, and PFM5 evidence slices, and found no implemented oracle drift or manifest parsing failure.

## Rollup Classification

| Checklist row | Current rollup state | PFM8 conclusion |
| --- | --- | --- |
| `Rust core library equivalent for core Stim semantics` | Rollup over active Rust APIs, transforms, DEMs, utilities, flows, analyzer, search, and sparse-tracker rows. | Keep `Partial`; PFM-B1, PFM-B3, and PFM-B4 are complete, while PFM-B2 plus PFM-B5 retain finite ledger-owned implementation work before PFM-B6 rollup. |
| `.stim`, `.dem`, and result-format compatibility` | `.stim` and implemented result-format paths are strong; the selected Rust DEM count, coordinate, transform, sampler, search, SAT/WCNF, and filter-key surfaces now have shared PFM-B3 traversal evidence. | Keep `Partial` only as a rollup over remaining PFM-B5 generated analyzer and search work; Python product shape, diagrams, and full ErrorMatcher provenance are deferred and do not keep the selected Rust DEM traversal child active. |
| `Full semantic execution of every legal circuit operation` | Selected sampler, detector-conversion, detection, analyzer, `SPP`, `SPP_DAG`, fixed-tableau, deterministic `MPP`, stochastic `MPP(p)` sampler or detection-sampling, deterministic `MPAD`, stochastic `MPAD(p)` sampler or detection-sampling, and noisy `MPAD(p)` analyzer evidence is green. PFM-B2 contract groundwork now classifies all 81 canonical gates across eight surfaces and every declared target-role pattern. | Keep `Partial`; eighteen exact, error-class, state-equivalence, structural, semantic-invariant, or statistical gate-contract cases remain planned and must gain independently selectable semantic evidence before this rollup can close. |
| `CLI binary` | Selected `stab` commands and selected legacy aliases are implemented with PF7 evidence. | Keep `Done for selected Stab CLI surface`; no stale PFM8 blocker found for the selected CLI surface. |
| `Highest-priority remaining feature gaps` | The section correctly lists active partial rollups and deferred surfaces. | Keep active rows `Partial` until their ledger-backed PFM-B cases are implemented or evidence-closed; deferred products remain separate. |

## PFM-B0 Blocker Ledger

PFM-B0 replaces broad under-specification with a schema-versioned, machine-checked ledger.
`just oracle::blockers` currently validates 130 cases across all eight open blocker families after PFM-B2 independently sourced deterministic MPP, anti-Hermitian MPP rejection, deterministic MPAD, stochastic MPP, and stochastic MPAD evidence and added identity-noise and control-flow owners.

| Blocker | Milestone | Decision | Cases | Planned | Implemented | Evidence close |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| PFM2 QEC transforms | PFM-B1 | Implement | 19 | 0 | 19 | 0 |
| PFM3 analyzer sweep | PFM-B2 | Evidence close | 1 | 0 | 0 | 1 |
| PFM3 gate execution | PFM-B2 | Implement | 18 | 18 | 0 | 0 |
| PFM4 DEM traversal | PFM-B3 | Implement | 7 | 0 | 7 | 0 |
| PFM5 detecting regions | PFM-B4 | Evidence close | 2 | 0 | 0 | 2 |
| PFM5 missing detectors | PFM-B4 | Evidence close | 14 | 0 | 0 | 14 |
| PFM5 flow engine | PFM-B4 | Implement | 33 | 0 | 33 | 0 |
| PFM6 analyzer and search | PFM-B5 | Implement | 36 | 14 | 22 | 0 |

The three evidence-close blocker records freeze 17 additional promoted supporting oracle rows: one analyzer CLI row, ten detecting-region rows, and six missing-detector rows.
The PFM-B4 flow blocker separately freezes four retained checker oracle rows, so 21 supporting oracle signatures are machine-bound across the ledger.
The ledger freezes ten supporting benchmark rows. The four PFM-B1 reverse-flow rows retain `contract-only` runner and comparability metadata with the `non-primary-report-only` threshold class; the six PFM-B4 rows cover four detecting-region rows, one missing-detector MPAD row, and one flow-checker batch row with `contract-only` runners, `non-primary-report-only` thresholds, and `report-only` comparability.
Thirteen owned cases outside PFM-B4 currently share a Rust selector with at least one other case. PFM-B1 and PFM-B4 have zero shared selectors; the remaining debt belongs to PFM-B5.

The validator rejects missing required blockers, any semantic change to the canonical SHA-256 ledger inventory, deleted owned-case floors, duplicate ids, unanchored or completed test-family aggregations, missing statistical plans, changed evidence-close supporting rows, supporting-oracle evidence-signature drift, pinned-golden path or SHA-256 drift, unsafe, untracked, or symlinked upstream paths, non-pinned Stim sources, non-implemented oracle rows, stale benchmark rows, typed oracle runner drift, distinct benchmark runner, threshold-class, or comparability drift, dishonest planned versus existing test state, completion claims backed only by planned artifacts, and unstable, non-regular, or oversized ledger, manifest, and upstream evidence inputs.
Schema version 2 additionally rejects missing or duplicate gate-surface and semantic-family coverage, checks those wire names against canonical core metadata, requires every PFM-B2 gate-family case to own parser, measurement-sampler, reference-sampler, detection-converter, detector-frame, detection-sampler, error-analyzer, and flow-generator evidence, and validates that the eighteen cases collectively own all nineteen canonical semantic families.
`just oracle::blockers --check-selectors` additionally proves every claimed existing selector resolves to at least one Rust test without executing arbitrary ledger commands; it rejects option-shaped filters and runs Cargo through the oracle harness's timeout and bounded-output controls. Direct Rust fixture execution separately requires at least one passed test, requires exactly one passed test for `--exact` rows, and rejects ignored-only evidence.

## Remaining Non-Deferred Blockers

The current blockers are no longer hidden broad upstream files or pending exact-subcase plans.
They are finite ledger-backed implementation and evidence-splitting programs:

- PFM-B1 is complete for the selected Rust transform scope: all nineteen QEC-transform cases have distinct selectors, exact or structural evidence, clean committed-HEAD allocation reports, final audit, and GPT-5.6/max review closure.
- PFM-B2 contract groundwork is complete and evidence-closes analyzer sweep behavior at the selected matrix; its final phase still implements eighteen planned cases covering all nineteen semantic families.
- PFM-B3 implements the seven selected shared folded-DEM traversal contracts.
- PFM-B4 is complete at `0f47eee04eacec96ed4e03dd36a18f58b76a0afc` for detecting regions, missing detectors, and all thirty-three flow cases. All milestone-audit and GPT-5.6/max findings are closed, and the matrix-solver plus sparse-repeat reports record clean committed-HEAD allocation evidence with zero resident delta.
- PFM-B5 implements the fourteen planned analyzer and SAT/WCNF cases and splits the thirteen implemented analyzer/search cases that still share selectors.

These exact ledger-owned items are the legitimate remaining work for the full GOAL and must be executed from `docs/plans/blocker-closure-ledger.json`, not reconstructed from checklist prose.
Broader repeat-contained feedback, future analyzer sweep shapes, and detecting-region or missing-detector behavior outside the selected evidence-close cases remain deferred or require an explicit plan revision; they are not active PFM8 blockers.

## Benchmarks

No benchmark gates changed in this PFM8 evidence slice.
The current report relies only on existing source-owned benchmark metadata and does not cite exploratory timing probes.
Before PFM8 can become a completion report, the commands in `docs/plans/GOAL.md` and the PFM8 section of `docs/plans/non-deferred-partial-feature-milestones.md` must be rerun from current committed `HEAD`, including primary timing and memory evidence if any benchmark gate changes.

## Audit Notes

This PFM8 pass confirms that the current rollup rows should stay conservative.
PFM-B2 contract groundwork is complete and recorded in `docs/plans/pfm-b2-gate-surface-contract-groundwork-report.md`.
PFM-B1 is complete with `19/19 implemented`, zero shared selectors, and four clean reports from `HEAD=4f193f19cebf132f7baf0a3aa1cc799a153a71ed` with `local_modifications=false`; its maximum peak live allocation is 84,280 bytes and maximum sampled resident delta is 8,192 bytes. PFM-B3 shared DEM traversal is complete at `4a984c26b39f6236fde5e3ff10cf0b42e8b155a2`, and PFM-B4 is complete at `0f47eee04eacec96ed4e03dd36a18f58b76a0afc`. PFM-B2 and PFM-B5 remain before PFM-B6 rollup.

## Verification

Completed in the original PFM8 evidence pass before this refresh:

- `cargo test -p stab-oracle fixtures --quiet`
- `cargo test -p stab-bench --quiet`
- `just oracle::list`
- `just oracle::matrix --check`
- `just oracle::run --implemented-only`
- `just bench::list`
- `just bench::smoke`
- `git diff --check`
- `git diff --cached --check`

The historical source-of-truth refresh subset below was rerun from clean committed `HEAD=0cf2d3eee423` with `local_modifications=false`:

- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::list`
- `just oracle::matrix --check`
- `just bench::list`
- `just bench::smoke`
- `just oracle::run --implemented-only`

That historical refresh predates later MPP and MPAD evidence and is not current PFM8 completion evidence.
During PFM-B0 on 2026-07-10, `just oracle::run --implemented-only` passed against the current worktree with `local_modifications=true`; PFM-B6 must regenerate final evidence from committed `HEAD` before this report can claim completion.

Still required before any final PFM8 completion claim:

- The authoritative final PFM8 checklist remains the Tests, Benchmarks, and Acceptance criteria sections in `docs/plans/non-deferred-partial-feature-milestones.md`.
- Every milestone-specific test listed in PFM1 through PFM7 must pass or be explicitly superseded by a newer, documented equivalent before the rollup can close.
- `cargo fmt --all --check`
- `cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `just oracle::blockers --check-selectors`
- `just oracle::run --implemented-only`
- `just bench::smoke`
- `just maintenance::pre-commit`
- Primary benchmark baseline, compare, timing-regression, and memory-regression commands when benchmark gates change or when the final rollup needs fresh primary evidence.
