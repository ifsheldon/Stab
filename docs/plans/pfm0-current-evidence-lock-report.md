# PFM0 Current Evidence Lock Report

## Summary

This PFM0 pass rechecked the current partial-feature closure state after the PF5 signed sampled flow slice, later PF2/PF5/PF6 evidence slices, the selected generated surface-code memory-X detecting-region evidence, and the selected PFM2 MPAD duplicate observable-id record parity slice.
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
- Active Rust and CLI work with exact owner milestones: PFM2 owns full circuit transform API parity and full feedback-inlining transform parity; PFM3 owns broader sweep-conditioned simulator and analysis parity plus remaining legal-gate execution support for implemented execution surfaces after the selected fixed-tableau, supported Hermitian `SPP` or `SPP_DAG`, deterministic `MPP`, deterministic `MPAD`, and selected noisy `MPAD(p)` analyzer boundary; PFM4 owns full DEM public API parity for DEM API, coordinate, transform, and folded-traversal gaps; PFM5 owns detector utilities, generated-code detector utility behavior, measurement-rich flows, and transform integration; PFM6 owns analyzer, search, sparse-tracker, and active matched-error value-object gaps.
- Under-specified active phrases that are not ready for implementation until exact subcases are selected. Current examples include broader analyzer sweep-shape parity beyond the selected PF3 matrix, broader legal non-tableau execution beyond the selected PF3 gate semantic boundary, broader `missing_detectors` utility families including generated-code suffix analysis beyond the pinned honeycomb and toric examples, broader repeat-contained feedback parity, broader QEC inverse and measurement-rich transform behavior, broader DEM folded traversal and coordinate behavior, broader detecting-region and gauge behavior, broader flow-generator, solver, diagnostic, folded-repeat, and transform-integration behavior, and broader PFM6 analyzer, search, sparse-tracker, and value-object hardening behavior, all logged in `docs/plans/milestone-spec-gaps.md`.
- Deferred-only surfaces that must not block the current Rust and CLI beta closure, including Python binding ergonomics, JS/WASM, diagram and rendering APIs, ecosystem integrations, simulator-product APIs, GPU, exact random-stream parity, C++ header compatibility, deprecated `--detector_hypergraph`, `explain_errors`, and `repl`.

The manifest-only PF rows remain extraction contracts, not implementation evidence.
They are acceptable only because each current implemented subcase is supplemented by executable exact, structural, property, statistical, or benchmark rows.

## Important Non-Implementation Finding

PFM4's unweighted SAT zero-probability behavior should not be "fixed" by simply skipping `error(0)` mechanisms.
The current `shortest_error_sat_problem` tests intentionally match Stim-style shortest-error semantics that ignore probabilities, including zero-probability error mechanisms, so skipping them would change parity instead of improving it.
The correct PFM4 work is true folded traversal or a sharper documented cap for the SAT consumer, not probability-based elision in the unweighted shortest-error problem.
The selected flat and nested zero-shift SAT repeat folding work preserves this boundary by folding unweighted repeated bodies structurally, including zero-probability mechanisms, where the compact folded body represents the same minimum structural parity cost without probability-based skipping.

## 2026-07-07 Addendum: PFM5 Inverted Record-Backed Observable Signed Flow

This addendum records a source-owned PF5 evidence-hardening slice for pinned Stim v1.16.0 `sample_if_circuit_has_stabilizer_flows_inverted_obs_rec`.
`sample_if_circuit_has_stabilizer_flows_checks_inverted_record_observables` now proves the Rust signed sampled-flow checker accepts `M !0` followed by `OBSERVABLE_INCLUDE(3) rec[-1]` for `-Z0 -> obs[3]` and rejects `Z0 -> obs[3]`.
The scope is locked in `docs/plans/pfm5-signed-sampled-flow-inverted-record-observable-scope.md`; it does not expand Python binding parity, exact random-stream parity, signed-flow diagnostics, or benchmark coverage.

## Tooling Evidence

The PFM0 metadata, manifest, benchmark-smoke, and implemented-oracle checks passed from clean committed `HEAD=3e305525bc9c` with `local_modifications=false` before this documentation refresh:

```sh
cargo test -p stab-oracle fixtures --quiet
just oracle::list
just oracle::matrix --check
just bench::list
just bench::smoke
just oracle::run --implemented-only
```

`just oracle::list` still lists red or manifest-only rows where expected, such as exact upstream help text and broad future extraction contracts.
The command was used here as a metadata/listing health check, not as a claim that every listed future row is implemented.

`just oracle::matrix --check` passed with 313 compatibility-matrix rows.
`cargo test -p stab-oracle fixtures --quiet` passed 45 fixture tests.
`just bench::list` parsed and listed the primary, report-only, non-primary, and PF placeholder benchmark metadata without adding PF report-only rows to the primary performance gate.
`just bench::smoke` passed with 153 planned benchmark rows.
`just oracle::run --implemented-only` passed, including the selected `pf2-inverse-qec-mpad-rust` row and the existing PF1 through PF7 executable evidence rows.

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
Do not select broader `missing_detectors` utility families, including generated-code suffix analysis, until a future plan names exact circuits or generated families, comparator behavior, resource behavior, oracle metadata, and benchmark policy.
Do not select broader PFM2 QEC inverse, PFM4 DEM folded traversal, PFM5 detector utility or flow, or PFM6 analyzer/search/sparse-tracker work from broad status prose alone; use `docs/plans/milestone-spec-gaps.md` to choose exact subcases before implementation.

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

The broader generated-code missing-detector suffix phrase is therefore logged as an open under-specification in `docs/plans/milestone-spec-gaps.md`, and the current selected generated-code suffix boundary is locked in `docs/plans/pfm5-missing-detectors-generated-boundary-scope.md`.
The broader non-generated `missing_detectors` utility families are now also logged as an open under-specification, so future agents should not implement or claim another missing-detector row until a plan selects exact circuits or generated families, suffix comparators or source-owned invariants, resource behavior, oracle metadata, and benchmark policy.
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

## 2026-07-07 Addendum: PFM2 Duplicate Measurement-Rich Time-Reversal Targets

This addendum records a PFM2 and PFM5 compatibility boundary for duplicate-target measurement-rich `time_reversed_for_flows`.
The selected measurement-rich time-reversal subset supports unique-target reset and measure-reset groups, including inverted measure-reset result targets, but rejects duplicate reset-only and duplicate measure-reset groups.
`time_reversed_for_flows_measurement_rich_subset_rejects_duplicate_reset_targets` and `time_reversed_for_flows_measurement_rich_subset_rejects_duplicate_measure_reset_targets` are the source-owned fail-closed evidence, selected by oracle row `pf2-time-reverse-flow-measurement-rust`.

Pinned Stim v1.16.0 probing recorded in `docs/plans/rpf5-flow-progress-report.md` returned malformed out-of-range inverse flows for duplicate reset-only and duplicate measure-reset examples.
Future work should not clone that behavior or silently "fix" it without a compatibility decision that chooses bug-compatible malformed flows, corrected semantic flows, or permanent rejection.
The current boundary is locked in `docs/plans/pfm2-time-reverse-duplicate-target-boundary-scope.md`.

## 2026-07-08 Addendum: PFM0 Broad Active-Wording Reconciliation

This addendum records a documentation-only PFM0 evidence-lock cleanup after the selected PFM2, PFM4, PFM5, and PFM6 exact subcases landed.
The broad roadmap phrases for remaining QEC inverse, measurement-rich transform, DEM folded traversal, coordinate traversal, detecting-region, `missing_detectors`, flow-generator, solver, analyzer, search, sparse-tracker, and matched-error hardening work are no longer treated as implementation-ready active scope.
They are now explicitly logged as open under-specification entries in `docs/plans/milestone-spec-gaps.md`.

No production behavior changed in this cleanup.
The purpose is to prevent the next implementation slice from reopening whole upstream files or vague feature families after the current exact evidence packets have already been promoted.
Future work in these areas must first name exact circuits or models, positive and negative tests, comparator behavior, resource boundaries, oracle metadata, benchmark policy or no-benchmark rationale, and documentation updates.

## 2026-07-08 Addendum: PFM2 MPAD Duplicate Observable-Id Evidence

This addendum records the selected PFM2 evidence-hardening slice committed as `3e30552 fix(core): merge duplicate MPAD observable records`.
The slice implements and proves record-only duplicate `OBSERVABLE_INCLUDE` id merging after MPAD-generated measurement records for the currently selected inverse-QEC boundary.
The source-owned Rust test evidence includes the MPAD inverse-QEC cases for duplicate observable ids, separated observable ids, existing-detector tails, and out-of-order observable ids, and oracle row `pf2-inverse-qec-mpad-rust` now selects the behavior.

The evidence is intentionally narrow.
It does not implement broader MPAD Pauli-observable tails, duplicate observable-id merging with non-record targets, repeat-contained MPAD/QEC inverse behavior, feedback-interleaved MPAD inverse behavior, or a full multi-instruction QEC inverse contract.
Those broader shapes remain governed by `docs/plans/milestone-spec-gaps.md` and require a future exact-subcase plan before coding.
