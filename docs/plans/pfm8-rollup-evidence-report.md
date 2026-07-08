# PFM8 Rollup Evidence Report

Date: 2026-07-08

Status: In progress, not a final PFM8 completion report.

## Scope

This report records the current PFM8 evidence state after the PFM0 evidence-lock cleanup committed as `1f80348 docs(plans): lock broad partial-feature scope` and the selected PFM2 MPAD duplicate observable-id record parity slice committed as `3e30552 fix(core): merge duplicate MPAD observable records`.
It covers the rollup layer only: `Rust core library equivalent for core Stim semantics`, `.stim`/`.dem`/result-format compatibility, `Full semantic execution of every legal circuit operation`, `Highest-priority remaining feature gaps`, and the selected CLI binary status.
It does not add production behavior, promote a new active feature subcase, or claim full Stim parity.

## Source-Of-Truth Inputs

- `docs/plans/GOAL.md` says the goal is complete only when every non-deferred partial row has implemented evidence or a named deferred subcase, documentation agrees with behavior, and milestone-audit plus full-code-review findings are fixed or logged as true under-specification.
- `docs/plans/non-deferred-partial-feature-milestones.md` says PFM8 may update rollup rows only after every active child row is implemented or explicitly deferred with a named reason.
- `docs/plans/partial-feature-inventory.md` maps current partial rows to active PFM owners, implemented child evidence, deferred-only exclusions, and manifest-only extraction contracts.
- `docs/plans/milestone-spec-gaps.md` now records the broad active wording that is not safe to implement without a future exact-subcase plan.
- `docs/stab-feature-checklist.md` remains the user-facing feature status document and still marks rollup or broad scoped rows as `Partial` where broader Stim parity is not proven.

## Current Evidence Snapshot

Implemented oracle evidence is healthy for the current selected Rust and CLI surface.
The original PFM8 snapshot passed `just oracle::run --implemented-only` on 2026-07-08 from `HEAD=1f80348` with `local_modifications=true` for docs-only PFM8 report and cross-link edits.
After the selected deterministic `MPAD` evidence row was added, `just oracle::run --implemented-only` passed again on 2026-07-08 from `HEAD=e8d16e722145` with `local_modifications=true` for the PF3 `MPAD` test, oracle metadata, and documentation synchronization edits, including `pf3-gate-mpad-execution-rust` plus the selected PF1 through PF7 executable rows for circuit APIs, gate metadata, transforms, sweep handling, DEM APIs, folded traversal subsets, detector utilities, measurement-rich flows, analyzer/search/sparse-tracker slices, selected CLI parity, and selected legacy dispatch.
After the selected noisy `MPAD(p)` analyzer evidence row was added, `just oracle::run --implemented-only` passed again locally with `local_modifications=true` for the analyzer fix, PF3 exact-output row `pf3-analyze-errors-mpad-noisy-cli`, and documentation synchronization edits.
After the selected deterministic `MPP` evidence row was added, PF3 structural oracle evidence passed on 2026-07-08 from `HEAD=efb4d47` with `local_modifications=true` for the PF3 `MPP` test, oracle metadata, and documentation synchronization edits, including `pf3-gate-mpp-execution-rust`.
After the selected PFM2 MPAD duplicate observable-id record parity slice was committed, `just oracle::run --implemented-only` passed on 2026-07-08 from clean committed `HEAD=3e305525bc9c` with `local_modifications=false`, including `pf2-inverse-qec-mpad-rust`.

Metadata evidence is healthy for the current manifests.
The current PFM8 verification pass reran oracle, matrix, and benchmark metadata checks after the PFM0 evidence-lock commit, the PF3 `MPP` and `MPAD` evidence updates, and the PFM2 MPAD duplicate observable-id record parity slice, and found no implemented oracle drift or manifest parsing failure.

## Rollup Classification

| Checklist row | Current rollup state | PFM8 conclusion |
| --- | --- | --- |
| `Rust core library equivalent for core Stim semantics` | Rollup over active Rust APIs, transforms, DEMs, utilities, flows, analyzer, search, and sparse-tracker rows. | Keep `Partial`; selected child evidence is healthy, but broader active under-specification remains in PFM2 through PFM6. |
| `.stim`, `.dem`, and result-format compatibility` | `.stim` and implemented result-format paths are strong; DEM behavior is scoped by active DEM API, folded traversal, analyzer, search, and sampler evidence. | Keep `Partial`; current implemented format paths are tested, but full DEM public API and folded traversal parity are not proven. |
| `Full semantic execution of every legal circuit operation` | Selected sampler, detector-conversion, detection, analyzer, `SPP`, `SPP_DAG`, fixed-tableau, deterministic `MPP`, deterministic `MPAD`, and noisy `MPAD(p)` analyzer evidence is green. | Keep `Partial`; broader legal non-tableau execution is explicitly under-specified until exact gate families and execution surfaces are selected. |
| `CLI binary` | Selected `stab` commands and selected legacy aliases are implemented with PF7 evidence. | Keep `Done for selected Stab CLI surface`; no stale PFM8 blocker found for the selected CLI surface. |
| `Highest-priority remaining feature gaps` | The section correctly lists active partial rollups and deferred surfaces. | Keep `Partial` rows; they should not move to `Done` while their remaining broad non-deferred work is under-specified rather than implemented or explicitly deferred. |

## Remaining Non-Deferred Blockers

The current blockers are no longer hidden broad upstream files.
They are named under-specification entries that require future exact-subcase plans before implementation:

- PFM2 broader QEC inverse, measurement-rich transform, and repeat-contained feedback behavior beyond the selected packets, including broader MPAD inverse-QEC shapes beyond selected record-only duplicate observable-id merging.
- PFM3 broader analyzer sweep-shape behavior and legal non-tableau execution beyond the selected sweep matrix, fixed-tableau execution, supported Hermitian `SPP` or `SPP_DAG`, deterministic `MPP`, deterministic `MPAD`, and selected noisy `MPAD(p)` analyzer boundary.
- PFM4 broader DEM folded traversal, coordinate traversal, and generated-loop analyzer-output dependencies beyond the selected consumers and documented caps.
- PFM5 broader detecting-region, `missing_detectors`, flow-generator, solver, diagnostic, folded-repeat, and transform-integration families beyond promoted evidence.
- PFM6 broader analyzer, search, SAT/WCNF, sparse-tracker, and matched-error value-object hardening families beyond selected exact slices.

These blockers are legitimate remaining work for the full GOAL.
They must not be turned into implementation tasks from checklist prose alone; each needs a scope note naming exact circuits or models, positive and negative tests, comparator behavior, resource boundaries, oracle metadata, benchmark policy or no-benchmark rationale, and documentation updates.

## Benchmarks

No benchmark gates changed in this PFM8 evidence slice.
The current report relies only on existing source-owned benchmark metadata and does not cite exploratory timing probes.
Before PFM8 can become a completion report, the commands in `docs/plans/GOAL.md` and the PFM8 section of `docs/plans/non-deferred-partial-feature-milestones.md` must be rerun from current committed `HEAD`, including primary timing and memory evidence if any benchmark gate changes.

## Audit Notes

This PFM8 pass confirms that the current rollup rows should stay conservative.
The correct next implementation step is not to mark broad rows done, but to choose one exact remaining subcase from `docs/plans/milestone-spec-gaps.md`, write a scope note, and then implement the GOAL work loop for that slice.

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

The latest PFM0 refresh subset below was rerun before this documentation update from clean committed `HEAD=3e305525bc9c` with `local_modifications=false`:

- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::list`
- `just oracle::matrix --check`
- `just bench::list`
- `just bench::smoke`
- `just oracle::run --implemented-only`

Still required before any final PFM8 completion claim:

- The authoritative final PFM8 checklist remains the Tests, Benchmarks, and Acceptance criteria sections in `docs/plans/non-deferred-partial-feature-milestones.md`.
- Every milestone-specific test listed in PFM1 through PFM7 must pass or be explicitly superseded by a newer, documented equivalent before the rollup can close.
- `cargo fmt --all --check`
- `cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `just oracle::run --implemented-only`
- `just bench::smoke`
- `just maintenance::pre-commit`
- Primary benchmark baseline, compare, timing-regression, and memory-regression commands when benchmark gates change or when the final rollup needs fresh primary evidence.
