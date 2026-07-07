# Partial Feature Closure Plan

Historical note: this document is retained as the first post-beta PF planning pass.
The active execution plan for remaining non-deferred partial rows is `docs/plans/non-deferred-partial-feature-milestones.md`, and `docs/plans/GOAL.md` now points at that plan.

## Summary

This plan covers every `Partial` row in `docs/stab-feature-checklist.md` whose remaining work is not intentionally deferred.
The purpose is to turn broad checklist gaps into implementation milestones with tests, benchmark evidence, acceptance criteria, and explicit exclusions.
It is not a plan for Python bindings, JS/WASM, diagrams, `explain_errors` CLI, `repl`, QASM, Quirk, Crumble, GPU, ecosystem packages, exact random-stream parity, C++ header compatibility, or public Python-style simulator APIs.

The checklist contains parent rows that are partial because they summarize many child surfaces.
Those parent rows are not separate implementation milestones.
They become complete only when the child rows below are complete, documented, and audited.

Use `docs/plans/lessons-learned.md` while executing this plan.
The most important lesson for this wave is to split each broad Stim feature into owned subcases, executable comparators, benchmark classes, resource limits, and documented deferrals before claiming progress.

## Scope Classification

The following table maps the active checklist partial rows into this plan.
Rows whose only remaining gap is intentionally deferred are recorded as exclusions so implementation agents do not accidentally broaden the beta scope.

| Checklist row or group | Plan owner | Included work | Excluded work |
| --- | --- | --- | --- |
| Rust core library equivalent for core Stim semantics | Rollup | Close when milestones PF1 through PF7 complete and public Rust surfaces are documented. | Python API clone, C++ header compatibility, graph/vector simulator products. |
| CLI binary | Rollup | Close when `stab m2d`, `stab analyze_errors`, and accepted legacy aliases have executable parity evidence. | Packaging a binary named `stim` unless a release plan adds it. |
| `.stim`, `.dem`, and result-format compatibility | Rollup | Close when active `.stim`, `.dem`, result-format, conversion, and analyzer gaps below are implemented or explicitly deferred. | Ecosystem export formats and diagrams. |
| Target kinds and full semantic execution | PF3 | Broader sweep-bit and legal-gate execution for Rust sampling, detection, conversion, and analyzer paths. | Exact random-stream parity and Python simulator APIs. |
| DEM parser, printer, counts, coordinates, transforms, and large-repeat traversal | PF4 | Rust DEM API parity for non-deferred construction, introspection, coordinates, flattening, rounding, tag stripping, and bounded or folded traversal. | DEM diagrams and Python operator ergonomics. |
| Gate validation flags, metadata, and semantic execution | PF1 and PF3 | Rust metadata needed for flow, tableau, decomposition, validation, and execution parity. | Python `GateData` class parity as a binding surface. |
| Circuit mutation, introspection, coordinates, repeats, transforms, reference samples, and determined measurements | PF1 and PF2 | Rust API closure for non-deferred methods plus transform semantics and resource boundaries. | QASM, Quirk, Crumble, diagrams, and Python operator ergonomics. |
| DEM construction, mutation, introspection, transforms, and analysis | PF4 and PF6 | Rust DEM methods needed by sampling, analyzer, search, and CLI workflows. | Match-graph diagrams and Python class operators. |
| Measurement-to-detection conversion | PF3 and PF7 | Remaining `m2d` and converter semantics for sweep-conditioned and feedback-related implemented surfaces. | Full Python converter class parity. |
| Detector-analysis utility APIs | PF5 | Detecting regions, missing detectors, feedback inlining, typed Rust APIs for owned subcases, and documented under-specification for broader repeat-contained feedback until exact subcases are selected. | Honeycomb or toric suffix parity unless specifically promoted by PF5 subcase extraction. |
| Circuit-to-DEM analysis and `analyze_errors` flags | PF6 and PF7 | Analyzer behavior, CLI flags, decomposition behavior, loop folding, and generated-circuit parity where owned by Rust/CLI. | `stim explain_errors` CLI and full ErrorMatcher provenance. |
| Shortest graphlike and hypergraph logical-error search | PF6 | Broader Rust search parity and generated-circuit evidence for non-deferred search APIs. | New public graph/vector simulator APIs. |
| Sparse reverse detector-frame tracking | PF6 | Optimized loop folding, deterministic generated supported-unitary coverage, and analyzer/search integration needed by implemented APIs. | Full ErrorMatcher provenance if it only serves deferred `explain_errors`. |
| Flows | PF5 | Measurement-rich flow solving and flow-transform support needed by Rust circuit APIs. | Python flow binding ergonomics. |
| `stim m2d` | PF7 | Visible CLI parity for supported input/output formats, sweep records, feedback inlining, resource boundaries, and error behavior. | `--detector_hypergraph`, which is intentionally excluded. |
| `stim analyze_errors` | PF7 | Visible CLI parity for the non-deferred analyzer surface. | `stim explain_errors` and diagram outputs. |
| Legacy top-level command flags | PF7 | Accepted aliases for implemented commands and conflict behavior. | Deprecated aliases not explicitly selected for Stab, including `--detector_hypergraph`. |

## Milestone PF0: Inventory Lock And Test Extraction

Objective: freeze the exact owned subcases before coding so no milestone relies on a whole upstream file or a vague checklist row.

Tasks:

- Re-read `docs/stab-feature-checklist.md`, `docs/stim-feature-list.md`, `docs/plans/stim-test-porting-plan.md`, and the pinned upstream tests under `vendor/stim`.
- Create or update `docs/plans/partial-feature-inventory.md` so it lists every active partial subcase, upstream source path, comparator type, oracle status, benchmark class, owner crate, and milestone PF1 through PF7.
- Split broad upstream files into `owned`, `semantic-mining`, `deferred`, and `out-of-scope` subcase groups before implementation begins.
- Add manifest-only oracle rows for owned subcases that lack executable fixtures, and add explicit future rows for subcases that remain deferred.
- Record any ambiguous scope in `docs/plans/milestone-spec-gaps.md` instead of weakening acceptance criteria.
- Update `docs/stab-feature-checklist.md` only if the current notes overstate support, understate support, or fail to identify deferred portions.

Tests:

- `just oracle::list`
- `just oracle::matrix --check`
- `cargo test -p stab-oracle fixtures --quiet`

Benchmarks:

- No performance runs are required.
- `just bench::list` must show planned rows or report-only placeholders for every milestone that needs benchmark evidence.

Acceptance criteria:

- Every active partial row in this plan has a milestone owner and at least one owned test or explicit reason why the first task is comparator implementation.
- `docs/plans/partial-feature-inventory.md` names active rows, rollup rows, and deferred-only partial rows without using a whole upstream file as acceptance evidence.
- No owned row uses a whole upstream source file as acceptance criteria.
- Every excluded surface names its deferral bucket or exclusion reason.

## Milestone PF1: Core Rust API And Metadata Closure

Objective: complete non-deferred Rust API gaps for circuit, gate, target, reference-sample, determined-measurement, and basic DEM ergonomics that are not binding-specific.

Included features:

- Circuit programmatic mutation: append text through the parser, clear, copy, insert, pop, concatenate, repeat, and file constructor or file writer helpers where they are useful Rust APIs. The implemented PF1 circuit API slices cover `clear`, Rust-native append-from-Stim-text parsing, the Stim Python compatibility alias name, atomic parse-failure handling for text append, bounded path-based `.stim` file reads, streaming `.stim` file writes, derived `Clone` copy semantics, circuit append and concatenation helpers, repeat special cases, nested repeat-count fusion, repeat overflow rejection, insertion with Stim-style boundary fusion, repeat-block insertion, and pop helpers.
- Circuit introspection: operation count, detector count, observable count, sweep-bit count, tick count, instruction-range views, and stable typed iterators. The implemented PF1 circuit API slices cover top-level length, emptiness, top-level item iteration, validated top-level item ranges, instruction-only ranges, lazy flattened forward and reverse instruction iteration through repeat blocks, measurement counts, detector counts, observable counts, tick counts, and sweep-bit counts.
- Circuit coordinates: final qubit coordinates and detector coordinates, including coordinate shifts through repeat blocks. The implemented PF1 circuit API slices cover folded final coordinate shifts, final qubit coordinates, all-detector coordinate maps, selected detector-coordinate maps, single-detector coordinate lookup, empty-coordinate detectors, and folded nested-repeat detector-coordinate queries.
- Reference samples and determined measurements: public Rust helpers for deterministic support used by sampler, detection, and analyzer flows. The implemented PF1 circuit API slice covers `Circuit::reference_sample`, `Circuit::reference_sample_tree`, and `Circuit::count_determined_measurements` as thin wrappers over the existing sampler, `ReferenceSampleTree`, and count-determined implementations, including sweep-controlled circuits under default-false reference semantics.
- Gate metadata: Rust-accessible metadata for aliases, inverse, category, target requirements, validation flags, flow, unitary/tableau, and decomposition where Stim semantics depend on it. The implemented PF1 slices cover aliases, argument rules, target rules, target grouping, fusing, noisy/reset/measurement/unitary/single-qubit/two-qubit/target-capability/symmetry flags, unitary inverse, generalized inverse, local Clifford tableau metadata, tableau-backed unitary flow metadata, fixed-shape one- or two-qubit `GateUnitaryMatrix` metadata, and H/S/CX/M/R `GateDecomposition` metadata; measurement-rich or variable-target flow data remains active work in PF5.
- DEM construction, mutation, and introspection basics: clear, append text or parsed instruction helpers, repeat helpers, counts, coordinates, typed item views, and ergonomic typed constructors that do not clone Python operators merely for shape. The implemented PF1 DEM API slices cover top-level length, emptiness, `clear`, programmatic instruction and repeat construction, push helpers, derived `Clone`, append-from-text, recursive tag stripping, folded final coordinate shifts, error counts, all-detector coordinate maps, selected-detector coordinate maps, single-detector lookup, top-level item iteration, validated item ranges, instruction-only ranges, typed item downcasts, lazy adjusted flattened instruction iteration, and exact-one-target validation for detector and logical observable declarations; rounded transforms, public materialized `flattened`, and full transform resource-boundary closure remain active PF4 work.

Tests:

- Port owned Rust semantic cases from `vendor/stim/src/stim/circuit/*_test.cc`, `vendor/stim/src/stim/gates/*_test.cc`, and Python API tests only as semantic-mining sources for method behavior.
- Add targeted tests for operation counts, detector and observable counts, sweep-bit counts, tick counts, coordinate shifts, repeat-block coordinate behavior, and typed iterator boundaries.
- Add negative tests for invalid mutation positions, invalid repeat counts, invalid text appends, invalid coordinates, unsupported gate metadata, and stale measurement references.
- Add parser round-trip tests proving file helper APIs preserve canonical `.stim` and `.dem` text.

Benchmarks:

- Add report-only or direct rows for high-volume introspection and coordinate queries: `pf1-circuit-coordinate-query`, `pf1-circuit-counts-repeat`, `pf1-dem-counts-repeat`, `pf1-dem-without-tags`, and `pf1-gate-metadata-lookup`.
- Classify rows as `direct-match` only when a faithful pinned Stim baseline exists; otherwise use `contract-representative` or `report-only`.

Acceptance criteria:

- Public Rust APIs use typed identifiers and domain errors after external parsing boundaries.
- New APIs are documented in Rust docs or matching project docs.
- Tests prove repeat-block and coordinate behavior without relying on Python bindings.
- Bench rows have manifest coverage, runner coverage, measurement work units, and compare notes.

## Milestone PF2: Circuit Transform And Repeat Traversal Closure

Objective: finish active circuit transform parity while preserving explicit deferrals for exports, diagrams, Python ergonomics, and keeping broader repeat-contained feedback beyond selected loop-refolding and nested bounded-repeat detector-parity cases under-specified until exact repeat structures, comparator behavior, resource behavior, oracle metadata, and benchmark policy are selected.

Included features:

- `flattened` and `flattened_operations` style Rust transform APIs for repeat blocks, tags, annotations, and measurement-index-sensitive instructions.
- `without_noise` for removing noise instructions while preserving deterministic circuit structure, coordinates, ticks, detectors, observables, and measurement record semantics.
- Broader `decomposed` parity for compound gates, MPP/SPP, pair measurements, and target grouping that can be validated with existing algebra and simulator components.
- Full Rust `with_inlined_feedback` behavior for selected single-control Pauli feedback and MPP cases, including selected bounded repeat-loop refolding, selected nested bounded-repeat detector-parity preservation, and explicit rejection of unsupported repeat-contained feedback.
- `time_reversed_for_flows` and measurement-rich flow-transform support when required by PF5 flow closure.
- Repeat traversal helpers that avoid accidental full expansion when a folded traversal can prove the requested transform.

Tests:

- Port owned cases from `vendor/stim/src/stim/circuit/circuit.test.cc`, `vendor/stim/src/stim/circuit/gate_decomposition.test.cc`, `vendor/stim/src/stim/util_top/transform_without_feedback.test.cc`, and flow-related tests selected during PF0.
- Add exact canonical-output tests for `flattened`, `without_noise`, supported `decomposed` cases, and feedback inlining.
- Add semantic tests that compare sampling, detector error models, or tableau action before and after transform where exact text is not sufficient.
- Add negative tests for unsupported repeat refolding, unsupported feedback controls, non-Clifford-like invalid shapes, unsupported decomposition targets, and resource-limit violations.

Benchmarks:

- Add rows `pf2-circuit-flatten-repeat`, `pf2-circuit-without-noise`, `pf2-circuit-decompose-mpp-spp`, `pf2-feedback-inline-batch`, and `pf2-time-reverse-flow`.
- Use paired submeasurements for mixed transform bundles, and gate only direct or CLI-comparable rows after repeated stable evidence.

Acceptance criteria:

- Every transform either matches pinned Stim v1.16.0 for owned cases or fails closed with a clear domain error for deferred cases.
- Measurement record references, detector references, observable references, and coordinate shifts are preserved or intentionally rewritten with tests.
- Resource behavior is explicit for large repeats: folded traversal where implemented, documented caps where expansion remains necessary, and rejection tests for caps.

## Milestone PF3: Sweep-Conditioned Execution And Gate-Semantic Coverage

Objective: close active gaps in sweep-bit target semantics and legal gate execution across sampler, detector conversion, detection sampling, and analyzer paths.

Included features:

- Extend sweep-conditioned semantics beyond the current `m2d` `CX`/`CY`/`CZ` detector-conversion subset where Stim-compatible Rust and CLI surfaces require it.
- Add sweep-aware detection sampling for implemented `detect` surfaces if PF0 classifies it as active rather than deferred.
- Add analyzer behavior for sweep targets when it affects public `analyze_errors` or Rust DEM generation outputs.
- Fill legal-gate execution gaps in sampler, converter, and analyzer paths for gates already accepted by the parser and not explicitly deferred.
- Preserve explicit rejections for unsupported sweep target shapes until their subcases are selected and tested.

Tests:

- Port owned sweep-bit and gate-semantics cases from `vendor/stim/src/stim/simulators/measurements_to_detection_events.test.cc`, `vendor/stim/src/stim/simulators/frame_simulator.test.cc`, `vendor/stim/src/stim/simulators/error_analyzer.test.cc`, and CLI tests for `detect` and `m2d`.
- Add parity tests for sweep records in `01`, `b8`, `r8`, `hits`, and `ptb64` where the format is accepted.
- Add semantic tests comparing sweep-conditioned detector conversion against explicit circuit expansion for small circuits.
- Add negative tests for sweep width mismatch, invalid sweep record counts, unsupported sweep target shapes, and sweep inputs combined with unsupported formats.

Benchmarks:

- Add rows `pf3-m2d-sweep-b8`, `pf3-m2d-sweep-ptb64-input`, `pf3-detect-sweep-sampling`, and `pf3-analyze-errors-sweep`, and keep `pf3-gate-semantic-wide` current as selected gate-semantic execution coverage expands.
- Classify CLI rows as `cli-baseline` when Stim v1.16.0 exposes the same command shape; classify core-only semantic rows as `contract-representative`.

Acceptance criteria:

- Sweep-conditioned paths have bounded or streaming input behavior and do not materialize all shots unless the API explicitly requests materialized output.
- CLI stdout, stderr class, exit status, and accepted flags match pinned Stim for owned cases.
- Gate execution support is documented by accepted/rejected subcase tables instead of implied by parser acceptance.

## Milestone PF4: DEM API, Transform, And Folded Traversal Closure

Objective: complete non-deferred DEM public Rust API parity and remove avoidable large-repeat expansion limits from DEM operations where practical.

Included features:

- DEM introspection for error counts, instruction counts, final detector shifts, final coordinate shifts, detector coordinate maps, and flattened iterators.
- DEM transforms `flattened`, `rounded`, and `without_tags` for owned cases.
- Folded traversal for graphlike search, hypergraph search, SAT encoding, analyzer-adjacent operations, matcher-adjacent operations, and shifted or otherwise non-selected repeated stochastic direct DEM sampling where current expansion or sampled-work limits are product limitations; flat sampled-error output and replay keep a separate Stim-compatible error-bit cap contract.
- Clear public behavior for decomposition separators, tags, coordinate shifts, repeat blocks, and numerical rounding.

Tests:

- Port owned cases from `vendor/stim/src/stim/dem/detector_error_model.test.cc`, `vendor/stim/src/stim/dem/dem_instruction.test.cc`, `vendor/stim/src/stim/dem/detector_error_model.pybind_test.py` as semantic mining, and DEM sampler tests where transform behavior affects sampling.
- Add exact canonical-output tests for rounded probabilities, tag stripping, flattening with coordinate shifts, and repeat blocks.
- Add structural tests for coordinate maps and final coordinate shifts.
- Add resource-boundary tests for large repeats, nested repeats, high detector shifts, high observable counts, and malformed DEMs.

Benchmarks:

- Add rows `pf4-dem-flatten-repeat`, `pf4-dem-rounded`, `pf4-dem-coordinate-map`, `pf4-dem-folded-graphlike-traversal`, and `pf4-dem-sampler-folded-repeat`.
- Use `direct-match` only for rows with faithful pinned Stim timing; otherwise use `contract-representative` or `report-only` with explicit notes.

Acceptance criteria:

- DEM APIs use typed detector ids, observable ids, coordinates, repeat counts, probabilities, and domain errors.
- Full expansion is not used for unbounded public CLI paths.
- Any remaining expansion cap is documented, tested, and recorded in the checklist as a product limitation instead of an accidental implementation detail.

## Milestone PF5: Detector Utilities And Flow Closure

Objective: finish active Rust utility APIs for detecting regions, missing detectors, feedback-related transforms, and measurement-rich flows.

Included features:

- Broader `circuit_detecting_regions` support for repeat-block traversal, additional Clifford gates, additional target shapes, tick windows, detector filtering, and gauge behavior selected during PF0.
- Broader `missing_detectors` support for multi-record detector row reduction, repeated MPP stabilizer-product cases, observable-interaction cases, honeycomb suffix cases, and toric suffix cases only if PF0 promotes exact subcases into active scope.
- Measurement-rich flow solving for `Flow`, `has_flow`, `has_all_flows`, `flow_generators`, and related circuit checks.
- Flow-aware transforms required by PF2, including time reversal for flows and feedback-inlining semantics.

Tests:

- Port owned cases from `vendor/stim/src/stim/util_top/circuit_to_detecting_regions.test.cc`, `vendor/stim/src/stim/util_top/missing_detectors.test.cc`, `vendor/stim/src/stim/stabilizers/flow.test.cc`, and Python flow tests as semantic-mining sources.
- Add positive tests for every promoted detecting-region gate and target shape.
- Add positive and negative tests for every promoted missing-detector family, including nondeterminism, multi-record detector rows, observables, repeated MPP products, and suffix-specific circuits if included.
- Keep the existing `coverage-stabilizers-flow` tests for basic `Flow` measurement indices, observables, multiplication, validation, and sign behavior current, and add new flow tests only for promoted `has_flow`, generator solving, solve-for-measurements, negative cases, transform integration, sampled signed checks, and diagnostics. The current unsigned diagnostic checker is covered separately by `pf5-has-flow-diagnostics-rust`, and the scoped sampled signed checker is covered by `pf5-signed-sampled-flows-rust`, so future diagnostic work should name the owning solver, generator, signed sampled diagnostic, or transform slice.

Benchmarks:

- Add rows `pf5-detecting-regions-repeat`, `pf5-missing-detectors-mpp`, `pf5-missing-detectors-generated-code`, `pf5-flow-solve-measurement-rich`, `pf5-flow-solve-measurement-python`, `pf5-has-all-flows-batch`, `pf5-flow-generators-measurement-rich`, and `pf5-flow-generators-measurement-python`.
- Keep complex utility rows report-only until faithful Stim comparison and stable ratios exist.

Acceptance criteria:

- Utility APIs fail closed for unpromoted subfamilies with precise errors.
- Flow APIs prove both positive and negative cases, including measurement-rich circuits and observable-including flows.
- If multi-record row reduction, honeycomb suffix analysis, or broader toric suffix analysis remains unpromoted, it is recorded as deferred in the checklist and spec-gap log.

## Milestone PF6: Analyzer, Search, And Sparse Reverse Tracking Closure

Objective: close non-deferred analyzer and logical-error search gaps without taking on the deferred `explain_errors` CLI or full ErrorMatcher provenance product.

Included features:

- Broader `circuit_to_detector_error_model` behavior for generated circuits, loop folding, gauge detectors, approximate disjoint errors, decomposition options, remnant-edge blocking, and ignored decomposition failures.
- Search parity for shortest graphlike errors, hypergraph logical-error search, and generated-circuit search cases selected during PF0.
- Sparse reverse detector-frame tracking improvements needed by analyzer and search parity, including optimized loop folding and generated coverage for each promoted unitary family.
- Existing matched-error value objects may be hardened when analyzer or search needs them, but full provenance is not required in this plan.

Tests:

- Port owned cases from `vendor/stim/src/stim/simulators/error_analyzer.test.cc`, `vendor/stim/src/stim/simulators/error_matcher.test.cc`, `vendor/stim/src/stim/search/*_test.cc`, `vendor/stim/src/stim/util_top/circuit_to_dem.test.cc`, and generated-circuit analyzer tests.
- Add exact `.dem` output tests for deterministic analyzer cases.
- Add structural tests for decomposed errors, gauge detectors, approximate disjoint handling, generated circuits, loop folding, and search results where exact ordering is not stable.
- Add generated or property-style tests for sparse reverse tracking when new supported unitary families, shifted repeated loops, detectors with coordinates, and observable frame changes are promoted.

Benchmarks:

- Add or extend rows `pf6-analyze-errors-generated-surface`, `pf6-error-decomp-loop-folded`, `pf6-graphlike-search-generated`, `pf6-hypergraph-search-generated`, `pf6-generated-sat-wcnf`, and `pf6-sparse-rev-frame-loop`.
- For bundled analyzer rows, use schema-version-2 submeasurement thresholds so a passing median cannot hide a slow decomposition or search subcase.

Acceptance criteria:

- Analyzer and search outputs match pinned Stim for owned exact cases and satisfy structural comparators for allowed-ordering cases.
- Loop folding is proven by tests and benchmarks rather than only by output equality on tiny circuits.
- Any remaining provenance, heralded matching, repeat-contained noise stack-frame, or `explain_errors` requirement stays explicitly deferred.

## Milestone PF7: Visible CLI Parity Closure

Objective: close active command-line gaps for `stab m2d`, `stab analyze_errors`, and accepted legacy dispatch without reintroducing excluded deprecated surfaces.

Included features:

- `stab m2d` parity for supported formats, `--sweep`, `--sweep_format`, `--ran_without_feedback`, `--skip_reference_sample`, `--append_observables`, `--obs_out`, `--obs_out_format`, streaming resource behavior, path errors, and format errors.
- `stab analyze_errors` parity for analyzer flags, input/output paths, stdout behavior, stderr class, exit status, gauge handling, decomposition flags, approximate disjoint errors, and fold-loop behavior.
- Legacy aliases for implemented commands: `--gen`, `--convert`, `--sample`, `--detect`, `--m2d`, and `--analyze_errors`, including conflict behavior when multiple modes are present.
- Documentation that `--detector_hypergraph` is intentionally excluded and users should use `stab analyze_errors` for the supported analyzer path.

Tests:

- Port owned cases from `vendor/stim/src/stim/cmd/command_m2d.test.cc`, `vendor/stim/src/stim/cmd/command_analyze_errors.test.cc`, `vendor/stim/src/stim/cmd/main_namespaced.test.cc`, and selected usage-document examples.
- Add exact oracle rows for common command shapes and malformed-input rows for stderr-class and exit-status behavior.
- Add resource-boundary tests for large `m2d` inputs, large sweep inputs, writer failure, invalid path handling, unsupported `ptb64` output, invalid observable side-output formats, and feedback inlining failures.
- Add CLI regression tests proving `--detector_hypergraph` is rejected or absent consistently.

Benchmarks:

- Add rows `pf7-cli-m2d-sweep-b8`, `pf7-cli-m2d-feedback-inline`, `pf7-cli-analyze-errors-generated`, `pf7-cli-analyze-errors-decompose`, and `pf7-cli-legacy-dispatch-startup`.
- Promote only direct pinned-Stim CLI rows with stable repeated evidence into the 1.25x threshold file.

Acceptance criteria:

- Command behavior is proven through oracle rows, not only core unit tests.
- CLI paths stream or document caps for public inputs and outputs.
- Help, README, roadmap, feature checklist, oracle manifest, benchmark manifest, and progress reports agree on the supported command surface.

## Milestone PF8: Benchmark Gate, Audit, And Documentation Closure

Objective: turn implementation evidence into durable acceptance evidence and update rollup rows without overstating parity.

Tasks:

- Run strict benchmark probes for every newly benchmarked row and record compare reports under `target/benchmarks/` with machine metadata, Stim metadata, Stab metadata, local-modification state, warmup state, and measurement-run count.
- Promote stable direct or CLI-comparable rows into `benchmarks/m12-primary-thresholds.json` at `max_relative_ratio: 1.25`.
- Keep report-only, contract-representative, proxy, tiny, or no-ratio rows out of the primary threshold file unless a later plan gives them source-owned waivers.
- Update profiler notes in the same change set as any threshold promotion.
- Update `docs/stab-feature-checklist.md`, `docs/plans/rust-stim-drop-in-rewrite.md`, benchmark docs, oracle metadata, and completion reports to match implemented behavior.
- Run milestone-audit and full-code-review for each milestone group before marking it complete.

Tests:

- `cargo test -p stab-bench --quiet`
- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::run --implemented-only`
- `just bench::smoke`
- Any milestone-specific test command listed in PF1 through PF7.

Benchmarks:

- `just bench::baseline --primary --out target/benchmarks/partial-feature-primary-baseline`
- `just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --baseline target/benchmarks/partial-feature-primary-baseline/baseline.json --report target/benchmarks/partial-feature-primary-compare`
- `just bench::primary-regression --baseline target/benchmarks/partial-feature-primary-baseline/baseline.json --report target/benchmarks/partial-feature-primary-regression`

Acceptance criteria:

- Every implemented milestone has a completion report or progress report that names tests, oracle rows, benchmark rows, audit outcome, review outcome, and remaining exclusions.
- Checklist parent rollups are updated only after child evidence exists.
- No report cites stale local evidence as authoritative release evidence.

## Required Final Verification

Before claiming this plan complete, run:

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings
cargo test --workspace --quiet
just oracle::run --implemented-only
just bench::smoke
just maintenance::pre-commit
```

If a milestone changes benchmark gates, also run the PF8 benchmark commands with a fresh baseline from current `HEAD`.
If a milestone changes public CLI behavior, include the relevant oracle command rows and CLI unit tests in the final evidence.
If a milestone changes public Rust APIs, update Rust docs or API docs in the same change set.

## Stop And Log Conditions

Stop implementation work and write a `docs/plans/milestone-spec-gaps.md` entry when:

- A subcase requires Python binding semantics, JS/WASM packaging, diagrams, QASM, Quirk, Crumble, GPU, ecosystem integrations, or exact random-stream parity.
- A whole upstream file is still being treated as acceptance criteria instead of split subcases.
- A CLI feature cannot define exact accepted flags, input formats, output formats, stdout behavior, stderr class, and exit status.
- A benchmark row cannot be classified as direct-match, cli-baseline, contract-representative, report-only, partial-match, or contract-only.
- A performance claim would require stale reports, local modifications that are not recorded, missing profiler notes, or informal waivers.
- A public parser, converter, sampler, analyzer, or writer path has neither streaming behavior nor a documented resource cap.
- A checklist update would need to hide a known limitation to mark a row done.
