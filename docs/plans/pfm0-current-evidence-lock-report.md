# PFM0 Current Evidence Lock Report

## Summary

This PFM0 pass rechecked the current partial-feature closure state after the PF5 signed sampled flow slice, later PF2/PF5/PF6 evidence slices, and the selected generated surface-code memory-X detecting-region evidence.
No production code change is selected by this report because the inspected candidate surfaces already have source-owned executable evidence, explicit owner milestones, deliberately scoped parity contracts, or an under-specification entry that must be resolved before implementation.

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
- `docs/plans/pfm5-detecting-regions-surface-memory-x-scope.md`.
- `docs/plans/rpf6-analyzer-progress-report.md`.
- Pinned Stim v1.16.0 source paths named by the touched inventory rows and feature inventory, including command, DEM, detector-conversion, analyzer, search, simulator, diagram, Python, JavaScript/WASM, and ecosystem source directories.
- Current oracle and benchmark manifests.

## Scope Classification

The current checklist still contains partial rows, but the main remaining blockers are no longer broad unclassified upstream files.
The remaining rows fall into these buckets:

- Rollup-only rows that depend on child evidence, such as the Rust core library equivalent, broad `.stim`, `.dem`, and result-format compatibility status, CLI binary regression status, and highest-priority feature-gap rollups. These rows may change status only after their active child rows have executable evidence or named deferrals.
- Active Rust and CLI work with exact owner milestones: PFM2 owns full circuit transform API parity and full feedback-inlining transform parity; PFM3 owns broader sweep-conditioned simulator and analysis parity plus remaining legal-gate execution support for implemented execution surfaces after the selected fixed-tableau and supported Hermitian `SPP` or `SPP_DAG` boundary; PFM4 owns full DEM public API parity for DEM API, coordinate, transform, and folded-traversal gaps; PFM5 owns detector utilities, generated-code detector utility behavior, measurement-rich flows, and transform integration; PFM6 owns analyzer, search, sparse-tracker, and active matched-error value-object gaps.
- Under-specified active phrases that are not ready for implementation until exact subcases are selected. Current examples are broader analyzer sweep-shape parity beyond the selected PF3 matrix, broader legal non-tableau execution beyond the selected PF3 gate semantic boundary, and broader generated-code `missing_detectors` suffix analysis beyond the pinned honeycomb and toric examples, all logged in `docs/plans/milestone-spec-gaps.md`.
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
- PFM2 or PFM3 active parity slices: a selected transform, feedback, time-reversal, sweep-conditioned execution, or legal-gate execution case with exact owned subcases and fail-closed unsupported shapes; for PF3 legal-gate execution, start from `docs/plans/pfm3-gate-semantic-boundary-scope.md` and update it only after selecting exact gate families, execution surfaces, comparator, resource behavior, oracle metadata, and benchmark policy.
- PFM5 detector utilities and flows: a named generated-code detector-utility slice, broader measurement-rich flow generator or solver diagnostic slice, or transform-integration slice with exact positive, negative, and resource-boundary tests.
- PFM6 analyzer/search: a named true folded generated-loop analyzer-output slice, broader loop-folded decomposition family, broader generated search/SAT/WCNF family, or sparse-tracker analyzer/search consumption slice.

Do not start from a whole upstream file.
The next slice must name exact owned subcases, explicit rejections, comparator class, oracle rows, benchmark rows or no-benchmark rationale, and resource behavior before implementation.
Do not select broader analyzer sweep-shape parity until a future plan names exact remaining gate-target shapes, comparator, CLI and Rust surfaces, oracle metadata, resource behavior, and benchmark policy.
Do not select broader legal non-tableau execution until a future plan names exact gate families, execution surfaces, comparator, resource behavior, oracle metadata, and benchmark policy.
Do not select broader generated-code `missing_detectors` suffix analysis until a future plan names exact generated families, suffix comparators, resource behavior, oracle metadata, and benchmark policy.

## 2026-07-07 Addendum: PF1 Spec-Gap Closure

This addendum resolves two stale PF1 specification gaps for the current Rust API scope without changing production behavior.
The path-based circuit file-helper boundary is no longer an active non-deferred gap because `Circuit::from_stim_file` is intentionally bounded by a 64 MiB read cap, `Circuit::write_stim_file` streams canonical output through IO, `pf1_circuit_file_helpers_read_and_write_canonical_stim_text` and `pf1_circuit_file_helpers_report_read_and_write_errors` prove the selected behavior, and oracle row `pf1-circuit-file-helpers` selects the evidence.
Unbounded streaming `.stim` file-read parity remains future parser work and must not block the current PF1 Rust API closure.

The Rust circuit coordinate non-finite behavior is also resolved for the current Rust API scope.
`pf1_circuit_stats_coordinate_queries_reject_non_finite_folded_shift` proves Stab rejects non-finite folded coordinate results, and the checklist, inventory, and PF1 progress report document that exact Python-style coordinate API shape plus exact C++ infinity side-effect parity remain deferred binding-compatibility work.
These closures keep the open under-specification log focused on active decisions rather than already documented deferrals.

## 2026-07-07 Addendum: PFM5 Generated Missing-Detector Boundary

This addendum records a PFM5 scope correction discovered while selecting the next implementation slice after the generated surface-code memory-X detecting-region evidence.
Pinned Stim v1.16.0 `src/stim/util_top/missing_detectors.test.cc` provides the honeycomb and toric generated-code suffix cases already promoted by `pf5-missing-detectors-generated-honeycomb-rust` and `pf5-missing-detectors-generated-toric-rust`.
No additional generated-code `missing_detectors` suffix family is currently named with an exact circuit, expected suffix, known-input mode, comparator, resource boundary, oracle row, or benchmark policy.

The broader generated-code missing-detector suffix phrase is therefore now logged as an open under-specification in `docs/plans/milestone-spec-gaps.md`, and the current selected generated-code suffix boundary is locked in `docs/plans/pfm5-missing-detectors-generated-boundary-scope.md`.
Future agents should not implement or claim another generated-code missing-detector row until a plan selects exact generated families and suffix comparators.
The existing report-only `pf5-missing-detectors-generated-code` benchmark remains scoped to the promoted honeycomb and toric workloads only.

## 2026-07-07 Addendum: PFM3 Analyzer Sweep-Shape Boundary

This addendum records a PFM3 scope correction discovered while selecting the next implementation slice after the generated missing-detector evidence lock.
Pinned Stim v1.16.0 `src/stim/simulators/error_analyzer.test.cc` contains the narrow `ErrorAnalyzer, ignores_sweep_controls` case for `CNOT sweep[0] 0`.
Current Stab evidence already promotes that case plus selected `CY`, `CZ`, `XCZ`, and `YCZ` no-ops, selected `CZ` sweep/sweep, record/sweep, sweep/record, and record/record classical-only no-op groups, public `stab analyze_errors` behavior, and invalid controlled-Pauli target-position rejections.
No additional analyzer sweep-shape family is currently named with exact gate-target shapes, expected no-op or rejection behavior, comparator, CLI or Rust surface, resource boundary, oracle row, or benchmark policy.

The broader analyzer sweep-shape phrase is therefore now logged as an open under-specification in `docs/plans/milestone-spec-gaps.md`, and the current selected matrix boundary is locked in `docs/plans/pfm3-analyzer-sweep-boundary-scope.md`.
Future agents should not implement or claim another analyzer sweep row until a plan selects exact remaining gate-target shapes and comparator policy.
The existing report-only `pf3-analyze-errors-sweep` benchmark remains scoped to the promoted analyzer sweep-control and `CZ` classical-only matrix only.

## 2026-07-07 Addendum: PFM4 Invalid DEM Search Target Boundary

This addendum records a PFM4 scope correction discovered while selecting a follow-up DEM search traversal slice.
Raw numeric `error` targets and separator-only `error` target lists are not valid public DEM search inputs in Stab.
`pf4_dem_public_validation_rejects_malformed_inputs` covers malformed text and typed programmatic constructor rejection, including invalid separators, separator-only target lists, and raw numeric error targets.
Oracle row `pf4-dem-validation-negative-rust` now names raw numeric error targets and separator-only target lists as validation-owned evidence.

The correct next step is not to add graphlike or hypergraph traversal for those malformed shapes.
Future PFM4 search traversal work should select valid DEM repeat bodies with detector or logical-observable targets, explicit comparator policy, oracle rows, benchmark rows or a no-benchmark rationale, and resource behavior.
The boundary is locked in `docs/plans/pfm4-dem-search-invalid-target-boundary-scope.md`.
