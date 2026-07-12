# PFM8 Rollup Evidence Report

Date: 2026-07-12

Status: PFM-B1 through PFM-B5 are complete for the selected Rust and CLI scope. PFM-B6 documentation synchronization is in progress pending final GPT-5.6/max re-review.

## Scope

This report rolls the finite blocker program in `docs/plans/non-deferred-partial-feature-milestones.md` into conservative feature status.
It covers selected Rust APIs, selected CLI behavior, gate execution, transforms, folded DEM traversal, detector utilities, flows, analyzer loop folding, graphlike and hypergraph search, SAT/WCNF output, sparse reverse tracking, and matched-error value objects.
It does not claim Python, JS/WASM, diagram, ecosystem, public interactive simulator, GPU, exact random-stream, C++ header, full ErrorMatcher provenance, deprecated `--detector_hypergraph`, or complete Stim product parity.

## Sources Of Truth

- `docs/plans/blocker-closure-ledger.json` is the executable finite case inventory.
- `docs/plans/non-deferred-partial-feature-milestones.md` defines PFM-B0 through PFM-B6 acceptance.
- `docs/plans/milestone-spec-gaps.md` records and resolves the planning loopholes revealed during implementation.
- `docs/stab-feature-checklist.md` is the user-facing status document.
- `docs/plans/partial-feature-inventory.md` maps selected child surfaces to tests, oracle rows, benchmarks, and deferrals.
- `docs/plans/pfm-b1-reverse-flow-progress-report.md`, `docs/plans/pfm-b3-folded-dem-traversal-progress-report.md`, `docs/plans/pfm-b4-detector-flow-progress-report.md`, `docs/plans/pfm-b5-analyzer-search-progress-report.md`, and `docs/plans/pfm-b2-gate-surface-progress-report.md` contain milestone evidence.

## Completion Snapshot

| Milestone | Selected scope | Completion evidence |
| --- | --- | --- |
| PFM-B1 | Reverse-flow and QEC transforms | 19 implemented cases, independent selectors, clean allocation reports from `HEAD=4f193f19cebf132f7baf0a3aa1cc799a153a71ed` |
| PFM-B2 | Analyzer sweep evidence and gate-by-surface execution | 1 evidence-closed analyzer case and 37 implemented gate cases; final-review fixes are committed through `fb47b03`, with fresh clean timing and allocation evidence pending |
| PFM-B3 | Shared folded DEM traversal | 7 implemented consumer contracts and clean allocation evidence from `HEAD=4a984c26b39f6236fde5e3ff10cf0b42e8b155a2` |
| PFM-B4 | Detecting regions, missing detectors, flow generation, checking, and solving | 16 evidence-closed detector-utility cases plus 33 implemented flow cases at `0f47eee04eacec96ed4e03dd36a18f58b76a0afc` |
| PFM-B5 | Generic analyzer folding, graphlike and hypergraph search, SAT/WCNF, sparse tracking, and matched errors | 52 implemented cases with final admission fix at `4c5901e2eaf03ddf0c8043b5655d943b70b92a70` |

PFM-B0's schema-version-2 ledger now validates eight blocker records and 165 cases.
Every implementation or evidence-close case has an independently resolving selector, no case remains planned, and the validator binds exact upstream provenance, oracle signatures, benchmark metadata, resource contracts, comparator classes, and canonical statistical plans.

## Rollup Classification

| Checklist row | PFM8 conclusion |
| --- | --- |
| `Rust core library equivalent for core Stim semantics` | `Done for selected Rust API scope`. Every non-deferred Rust blocker is closed; explicitly deferred product APIs remain outside this status. |
| `.stim`, `.dem`, and result-format compatibility | Keep `Partial` only as a literal full-product rollup. Selected Rust file-format, result-format, traversal, analyzer, search, and gate semantics are done; named deferred commands, bindings, diagrams, and provenance products prevent a full-Stim claim. |
| `Gate semantic execution` | `Done for selected Rust/CLI scope`. PFM-B2 classifies all 81 canonical gates across eight surfaces and closes all nineteen semantic families through 37 independent cases. |
| `CLI binary` | `Done for selected Stab CLI surface`. Selected commands and aliases are implemented; drop-in `stim` packaging and deferred commands remain excluded. |
| `Highest-priority remaining feature gaps` | No non-deferred blocker remains in the source ledger. Future work must be introduced by a new exact plan instead of broadening a completed row. |

## Gate And Analyzer Closure

PFM-B2's canonical contract has no unknown or implicit fallback state.
It maps 81 gates, nineteen semantic families, 22 accepted target patterns, and parser-accepted target groups across parser, measurement sampler, reference sampler, detection converter, detector frame, detection sampler, error analyzer, and flow generator.
The final review remediation strengthens fixed-tableau inverse equivalence, grouped pair-measurement inversion, pinned four-body MPP invariants, controlled-Pauli feedback and sweep directionality, independent noisy-gate effects, heralded records, anti-Hermitian rejection, exact upstream anchors, statistical-catalog ownership, and hostile-ledger work bounds.

`just oracle::blockers --check-selectors` reports:

| Blocker | Milestone | Cases | Planned | Implemented | Evidence close |
| --- | --- | ---: | ---: | ---: | ---: |
| PFM2 QEC transforms | PFM-B1 | 19 | 0 | 19 | 0 |
| PFM3 analyzer sweep | PFM-B2 | 1 | 0 | 0 | 1 |
| PFM3 gate execution | PFM-B2 | 37 | 0 | 37 | 0 |
| PFM4 DEM traversal | PFM-B3 | 7 | 0 | 7 | 0 |
| PFM5 detecting regions | PFM-B4 | 2 | 0 | 0 | 2 |
| PFM5 missing detectors | PFM-B4 | 14 | 0 | 0 | 14 |
| PFM5 flow engine | PFM-B4 | 33 | 0 | 33 | 0 |
| PFM6 analyzer and search | PFM-B5 | 52 | 0 | 52 | 0 |

## Benchmark Evidence

No primary runner or 1.25x threshold changed during final PFM-B2 review, so no fresh primary-gate run is required.
The earlier `target/benchmarks/pfm-b2-final-reviewed-*` reports at `6576273` are superseded because final review changed the sweep-reference hot path and split detector-frame timing from ordinary detection sampling.
Fresh `target/benchmarks/pfm-b2-closure-*` reports must identify one committed revision with `local_modifications=false`, warmup enabled, and three measurement runs.
The gate row must report sampler execution, reference sampling, converter compilation, ordinary detection sampling, forced detector-frame sampling, error analysis, and flow generation separately, omit its heterogeneous row median, and render every normalized submeasurement.
The gate and analyzer-sweep rows remain contract-only and report-only, so no Stab/Stim ratio or beta-gate claim is made.

PFM-B5's faithful graphlike comparison rows remain report-only at 4.647x and 4.279x Stim.
Those optimization backlogs do not invalidate semantic closure and remain outside the primary threshold file.

## Audit And Review

Milestone audits for PFM-B1, PFM-B3, PFM-B4, and PFM-B5 are complete in their progress reports.
The PFM-B2 audit found semantic, provenance, allocation, benchmark-diagnostic, statistical-resource, and modularity defects in the first completion evidence; commits `f1f6e42`, `6bdff8b`, and `6576273` close that round.
Final GPT-5.6/max review then found a sweep-reference copy, duplicated surface schema, floating statistical-boundary drift, non-exact GTest gate markers, hidden detector-frame timing, and a heterogeneous report median; commits `2f46c33`, `25f352b`, `8ab85e4`, and `fb47b03` close those findings.
The exact-provenance loophole and final gate-scope boundary are resolved in `docs/plans/milestone-spec-gaps.md`.
Fresh clean benchmark evidence and follow-up GPT-5.6/max re-review of the remediation remain blocking until complete.

## Verification

The current PFM-B2/PFM-B6 pass completed:

- `cargo fmt --all --check`
- `cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `cargo test -p stab-core --features ops-contracts warmed_fixed_tableau_gate_execution_does_not_allocate_per_dispatch --quiet`
- `cargo test -p stab-core --features ops-contracts streamed_sweep_conversion_adds_no_per_shot_scratch_allocations --quiet`
- `just oracle::blockers --check-selectors`
- `just oracle::run --milestone PF3`
- `just oracle::run --milestone M8`
- `just oracle::run --implemented-only`
- `just bench::smoke`
- clean PFM-B2 timing and allocation commands recorded in `docs/plans/pfm-b2-gate-surface-progress-report.md`

Final documentation commit verification must rerun `just maintenance::pre-commit` and the standard checks after resolving any final review findings.

## Explicitly Deferred Work

The following work remains future scope and is not an active blocker: Python bindings and Python object shape, JS/WASM, diagrams, `repl`, QASM, Quirk, Crumble, ecosystem packages, GPU, exact random-stream parity, public graph or vector simulator products, C++ header compatibility, full ErrorMatcher provenance, `explain_errors`, deprecated `--detector_hypergraph`, and behavior outside the finite selected ledger.
Broader repeat-contained feedback, future analyzer sweep shapes, and detector-utility behavior outside evidence-closed cases require a new exact plan before implementation.
