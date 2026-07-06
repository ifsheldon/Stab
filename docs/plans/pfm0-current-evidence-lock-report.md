# PFM0 Current Evidence Lock Report

## Summary

This PFM0 pass rechecked the current partial-feature closure state after the PF5 signed sampled flow slice.
No production code change was selected because the inspected candidate surfaces already had source-owned executable evidence, explicit owner milestones, or deliberately scoped parity contracts.

This is not a final PFM8 completion report.
It is a current-state evidence lock for scope classification, metadata health, and next implementation selection.

## Sources Rechecked

- `docs/plans/GOAL.md`.
- `docs/plans/lessons-learned.md`.
- `docs/stab-feature-checklist.md`.
- `docs/stim-feature-list.md`.
- `docs/plans/non-deferred-partial-feature-milestones.md`.
- `docs/plans/partial-feature-inventory.md`.
- `docs/plans/milestone-spec-gaps.md`.
- `docs/plans/rpf2-circuit-transform-progress-report.md`.
- `docs/plans/rpf3-sweep-gate-progress-report.md`.
- `docs/plans/rpf4-dem-search-sat-progress-report.md`.
- `docs/plans/rpf4-dem-coordinate-progress-report.md`.
- `docs/plans/rpf5-missing-detectors-progress-report.md`.
- `docs/plans/rpf6-analyzer-progress-report.md`.
- Pinned Stim v1.16.0 source paths named by the touched inventory rows and feature inventory, including command, DEM, detector-conversion, analyzer, search, simulator, diagram, Python, JavaScript/WASM, and ecosystem source directories.
- Current oracle and benchmark manifests.

## Scope Classification

The current checklist still contains partial rows, but the main remaining blockers are no longer broad unclassified upstream files.
The remaining rows fall into these buckets:

- Rollup-only rows that depend on child evidence, such as the Rust core library equivalent, broad `.stim`, `.dem`, and result-format compatibility status, CLI binary regression status, and highest-priority feature-gap rollups. These rows may change status only after their active child rows have executable evidence or named deferrals.
- Active Rust and CLI work with exact owner milestones: PFM2 owns full circuit transform API parity and full feedback-inlining transform parity; PFM3 owns broader sweep-conditioned simulator and analysis parity plus remaining legal-gate execution support for implemented execution surfaces; PFM4 owns full DEM public API parity for DEM API, coordinate, transform, and folded-traversal gaps; PFM5 owns detector utilities, generated-code detector utility behavior, measurement-rich flows, and transform integration; PFM6 owns analyzer, search, sparse-tracker, and active matched-error value-object gaps.
- Deferred-only surfaces that must not block the current Rust and CLI beta closure, including Python binding ergonomics, JS/WASM, diagram and rendering APIs, ecosystem integrations, simulator-product APIs, GPU, exact random-stream parity, C++ header compatibility, deprecated `--detector_hypergraph`, `explain_errors`, and `repl`.

The manifest-only PF rows remain extraction contracts, not implementation evidence.
They are acceptable only because each current implemented subcase is supplemented by executable exact, structural, property, statistical, or benchmark rows.

## Important Non-Implementation Finding

PFM4's unweighted SAT zero-probability behavior should not be "fixed" by simply skipping `error(0)` mechanisms.
The current `shortest_error_sat_problem` tests intentionally match Stim-style shortest-error semantics that ignore probabilities, including zero-probability error mechanisms, so skipping them would change parity instead of improving it.
The correct PFM4 work is true folded traversal or a sharper documented cap for the SAT consumer, not probability-based elision in the unweighted shortest-error problem.
The selected flat and nested zero-shift SAT repeat folding work preserves this boundary by folding unweighted repeated bodies structurally, including zero-probability mechanisms, where the compact folded body represents the same minimum structural parity cost without probability-based skipping.

## Tooling Evidence

The PFM0 metadata and manifest checks passed from the current worktree:

```sh
cargo test -p stab-oracle fixtures --quiet
just oracle::list
just oracle::matrix --check
just bench::list
```

`just oracle::list` still lists red or manifest-only rows where expected, such as exact upstream help text and broad future extraction contracts.
The command was used here as a metadata/listing health check, not as a claim that every listed future row is implemented.

`just oracle::matrix --check` passed with 313 compatibility-matrix rows.
`cargo test -p stab-oracle fixtures --quiet` passed 45 fixture tests.
`just bench::list` parsed and listed the primary, report-only, non-primary, and PF placeholder benchmark metadata without adding PF report-only rows to the primary performance gate.

## Next Implementation Candidates

The next implementation slice should be selected from one of these active, source-owned gaps:

- PFM4 folded DEM traversal: broader folded graphlike, hypergraph, SAT/WCNF, analyzer, matcher-adjacent, or sampled-error traversal work beyond the selected graphlike, hypergraph, ErrorMatcher filter DEM, SAT/WCNF, and sampler repeat folds already recorded, while preserving Stim-compatible unweighted shortest-error semantics and existing dense target caps.
- PFM2 or PFM3 active parity slices: a selected transform, feedback, time-reversal, sweep-conditioned execution, analyzer sweep-shape, or legal-gate execution case with exact owned subcases and fail-closed unsupported shapes.
- PFM5 detector utilities and flows: a named generated-code detector-utility slice, broader measurement-rich flow generator or solver diagnostic slice, or transform-integration slice with exact positive, negative, and resource-boundary tests.
- PFM6 analyzer/search: a named true folded generated-loop analyzer-output slice, broader loop-folded decomposition family, broader generated search/SAT/WCNF family, or sparse-tracker analyzer/search consumption slice.

Do not start from a whole upstream file.
The next slice must name exact owned subcases, explicit rejections, comparator class, oracle rows, benchmark rows or no-benchmark rationale, and resource behavior before implementation.
