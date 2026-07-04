# Non-Deferred Partial Feature Milestones

## Summary

This plan covers every feature row that is marked `Partial` in `docs/stab-feature-checklist.md` and still has non-deferred Rust or CLI work.
It excludes rows whose remaining work is only Python bindings, JavaScript/WASM, diagrams, ecosystem integrations, public simulator products, C++ header compatibility, exact random-stream parity, or deprecated `--detector_hypergraph` support.

This is the active planning document for finishing the remaining partial feature surfaces.
Use `docs/plans/remaining-partial-feature-milestones.md`, `docs/plans/partial-feature-inventory.md`, and existing RPF progress reports as historical context and source material, but execute this document when deciding the next implementation packet.

The main planning rule is simple: a row may not move from `Partial` to `Done` because nearby functionality exists.
It moves only when the exact active subcases have implementation, tests, oracle evidence where relevant, benchmark evidence where relevant, synchronized documentation, milestone-audit closure, and full-code-review closure.

## Scope

Included:

- Rust core APIs and internal execution surfaces that are partial and not intentionally deferred.
- Stab CLI behavior for implemented or selected Stim-compatible commands.
- `.stim`, `.dem`, and result-format behavior needed by the active Rust and CLI surfaces.
- Tests, oracle fixtures, benchmark rows, profiler notes, reports, and documentation needed to prove the active surfaces.
- Explicit fail-closed behavior for unsupported shapes that remain outside a milestone.

Excluded:

- Python bindings and Python API ergonomics.
- JavaScript/WASM.
- Diagrams and visualization.
- `stim explain_errors` CLI.
- `stim repl`.
- QASM, Quirk, Crumble, `stimcirq`, `sinter`, ZX, lattice-surgery glue, and other ecosystem packages.
- GPU backends.
- Exact random-stream parity.
- C++ header compatibility.
- New public graph simulator, vector simulator, `TableauSimulator`, or `FlipSimulator` products.
- Deprecated `--detector_hypergraph` support.
- Generated API documentation and machine-readable feature-matrix tooling, because that row is `Missing` instead of `Partial`.

If a subcase in this plan turns out to require an excluded surface, stop and log the under-specification in `docs/plans/milestone-spec-gaps.md` instead of silently widening scope.

## Covered Partial Rows

| Plan milestone | Checklist rows covered | Notes |
| --- | --- | --- |
| PFM0 | Programmatic mutation, core introspection, circuit coordinate queries, reference samples and determined measurements, DEM construction and mutation, rollup rows | Reconcile rows that are partial mostly because deferred Python or product surfaces are absent, and split any remaining active Rust subcases before implementation. |
| PFM1 | Gate validation flags and categories, gate semantic execution, full semantic execution of every legal circuit operation, flows | Close metadata and execution-support contract gaps, including the resolved measurement-rich and variable-target flow metadata contract. |
| PFM2 | Repeat handling, circuit transforms, measurement-to-detection conversion, full circuit transform API parity, full feedback-inlining transform parity | Finish flow-aware transforms, feedback-loop decisions, repeat traversal behavior, and transform resource boundaries. |
| PFM3 | Target kinds, gate semantic execution, measurement-to-detection conversion, broader sweep-conditioned simulator and analysis parity | Finish or precisely reject remaining sweep-conditioned execution and analyzer subcases. |
| PFM4 | DEM parser and canonical printer, DEM detector shifts, DEM introspection, DEM transforms, DEM flattening and large repeat traversal, full DEM public API parity | Finish DEM API gaps and folded or capped traversal behavior for selected consumers. |
| PFM5 | Detector-analysis utility APIs, flows, circuit transforms, gate validation flags and categories | Finish detecting regions, missing detectors, measurement-rich flow solving, and flow-driven transform integration. |
| PFM6 | Circuit-to-DEM analysis, `analyze_errors --decompose_errors`, DEM analysis and shortest graphlike error, shortest graphlike and hypergraph search, sparse reverse detector-frame tracking, active matched-error value objects | Finish analyzer/search/sparse-tracker gaps without taking on full ErrorMatcher provenance or `explain_errors` CLI. |
| PFM7 | `stim m2d`, `stim analyze_errors`, legacy top-level command flags, CLI binary | Finish visible CLI parity for selected commands and accepted legacy aliases, with `--detector_hypergraph` remaining excluded. |
| PFM8 | Rust core library equivalent, CLI binary, `.stim`/`.dem`/result-format compatibility, full semantic execution, highest-priority remaining feature gaps | Audit, review, benchmark, documentation, and rollup-status closure after child milestones have evidence. |

## Execution Order

Recommended order:

1. Run PFM0 before each new wave if the checklist, inventory, or roadmap has changed.
2. Run PFM1 and PFM5 before measurement-rich flow-dependent PFM2 work, because measurement-rich `time_reversed_for_flows` and flow-aware decomposition checks need measurement-rich flow semantics.
3. Run PFM3 before PFM7 when CLI `m2d` or `detect` work depends on core sweep behavior.
4. Run PFM4 before PFM6 when analyzer or search work depends on DEM folded traversal behavior.
5. Run PFM8 only after one or more implementation milestones have fresh source-owned evidence.

Milestones may be implemented in smaller slices, but every slice must name the checklist rows, oracle rows, benchmark rows, deferred edges, and done criteria it owns.

## Required Packet For Every Implementation Slice

Each PFM1 through PFM7 slice must include:

- A scope note naming the exact implemented subcases, explicit rejections, explicit deferrals, comparator class, resource behavior, oracle rows, and benchmark rows.
- Tests before or alongside implementation, covering positive behavior, negative behavior, malformed input, resource boundaries, compatibility-sensitive edge cases, and unsupported-shape errors.
- Oracle evidence when the surface has public compatibility semantics or when an existing manifest-only row needs executable evidence.
- Benchmark evidence when the surface is performance-sensitive, including measurement work units, compare notes, runner coverage or an explicit placeholder, and primary-gate or report-only classification.
- Documentation updates in the same change set, including the checklist, this plan or a progress report, roadmap text, oracle metadata, benchmark metadata, and user-facing docs when behavior changes.
- Milestone-audit closure, with implementation findings fixed and true under-specification findings logged in `docs/plans/milestone-spec-gaps.md`.
- Full-code-review closure, with GPT-5.5/xhigh subagents spawned during Codex review work when available.

Do not mark a milestone complete if any packet item is missing.

## PFM0: Scope Reconciliation And Evidence Lock

Objective: lock the exact active subcases before implementation resumes and prevent deferred-only gaps from polluting active milestones.

Rows covered:

- All `Partial` rows in `docs/stab-feature-checklist.md`.
- Classification-heavy rows such as programmatic mutation, core introspection, circuit coordinate queries, reference samples, DEM construction, single-shot simulator, flip-frame simulator, and generated API docs.
- Rollup rows that depend on active child rows.

Tasks:

- Re-read `docs/stab-feature-checklist.md`, `docs/stim-feature-list.md`, `docs/plans/partial-feature-inventory.md`, `docs/plans/lessons-learned.md`, `docs/plans/milestone-spec-gaps.md`, and the pinned Stim v1.16.0 source for any row being touched.
- For every partial row, classify the remaining work as active Rust/CLI work, deferred-only work, missing-tooling work, or rollup-only work.
- Update `docs/plans/partial-feature-inventory.md` when a row still relies on broad upstream files or stale wording.
- Update `docs/stab-feature-checklist.md` when a row is partial only because deferred surfaces are absent.
- Add or refresh manifest-only oracle rows only for active subcases that need future executable evidence.
- Add or refresh non-primary benchmark placeholders only for performance-sensitive active subcases.
- Log any unresolved scope question in `docs/plans/milestone-spec-gaps.md`.

Tests:

- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::list`
- `just oracle::matrix --check`
- `just bench::list`

Benchmarks:

- No timing run is required.
- Metadata must parse, list, and keep placeholder rows out of the primary benchmark gate.

Acceptance criteria:

- No active milestone treats a whole upstream file as its acceptance criterion.
- Every active row has an exact owner milestone and comparator plan.
- Every deferred-only row has a named deferral reason.
- Every rollup row names the child milestones that must close before the rollup status changes.

## PFM1: Gate Metadata And Execution-Support Contract

Objective: finish active gate metadata gaps and make parser acceptance, metadata availability, and execution support impossible to confuse.

Rows covered:

- Gate validation flags and categories.
- Gate semantic execution.
- Full semantic execution of every legal circuit operation, for gate-table and execution-support bookkeeping.
- Flows, for gate-level flow metadata decisions.

Tasks:

- Refresh `docs/plans/rpf1-gate-execution-support-contract.md` so every canonical Stim v1.16.0 gate records validation support, tableau metadata, unitary metadata, flow metadata, decomposition metadata, sampler execution, detector conversion, analyzer propagation, and explicit rejection behavior.
- Keep the resolved decision that measurement-rich and variable-target `GateData.flows` metadata belongs in `Gate::flows`, while sampler, detector-conversion, analyzer, and full circuit flow execution support remain separate milestone surfaces.
- Implement typed accessors or precise unsupported-accessor errors for any active metadata shape.
- Keep `SPP` and `SPP_DAG` parser, decomposition metadata, sampler execution, detection-conversion execution, and analyzer execution behavior synchronized.
- Document parser acceptance separately from execution support in the checklist and support contract.

Tests:

- Add table-driven tests that compare every canonical gate against the support contract.
- Port owned metadata cases from `vendor/stim/src/stim/gates/gates.test.cc`, `vendor/stim/src/stim/gates/gates_test.py`, and gate data tests as semantic sources.
- Add positive tests for decomposition metadata, tableau metadata, unitary metadata, flow metadata, aliases, inverses, and target-grouping accessors.
- Add negative tests for unsupported metadata on noisy, annotation, `MPAD`, and shape-dependent gates, and execution-boundary tests for parser-accepted metadata gates whose execution remains unsupported.
- Add execution-boundary tests proving parser-accepted but unsupported execution gates fail with precise domain errors in sampler, detector conversion, and analyzer paths.

Oracle rows:

- Keep implemented rows such as `pf1-gate-decomposition-metadata` current if public API names or behavior change.
- Add a structural row for the canonical execution-support table if the table gains executable validation beyond ordinary unit tests.

Benchmarks:

- Extend `pf1-gate-metadata-lookup` if metadata accessors change materially.
- Keep `pf3-gate-semantic-wide` current when execution support changes.
- Keep metadata rows report-only unless a faithful direct Stim baseline and repeated stable evidence exist.

Acceptance criteria:

- Every canonical gate has explicit metadata availability and execution behavior.
- Unsupported metadata and execution paths fail closed with typed errors.
- Documentation no longer implies that parsing a gate means every execution surface supports it.

## PFM2: Circuit Transforms And Flow-Aware Rewrites

Objective: finish active circuit transform gaps while keeping exports, diagrams, and Python ergonomics excluded.

Rows covered:

- Repeat handling.
- Circuit transforms.
- Measurement-to-detection conversion, for feedback-inlining prerequisites.
- Full circuit transform API parity.
- Full feedback-inlining transform parity.

Tasks:

- Finish flow-semantic checks for `Circuit::decomposed` that depend on PFM5 measurement-rich flows.
- Promote `time_reversed_for_flows` beyond the current unitary Rust subset for the measurement-rich flow cases selected by PFM5, or write an explicit spec-gap entry if the public Rust API shape is still under-specified.
- Decide whether exact feedback loop refolding is active Rust scope; if yes, implement it for the selected repeat-block feedback cases, and if no, preserve precise repeat-block rejection with tests and documentation.
- Preserve measurement record, detector, observable, coordinate, sweep, and repeat semantics across every transform.
- Keep materialized transforms capped or folded, and reject excessive expansion before building huge intermediate circuits.

Tests:

- Port owned transform cases from `vendor/stim/src/stim/circuit/circuit.test.cc`, `vendor/stim/src/stim/circuit/gate_decomposition.test.cc`, `vendor/stim/src/stim/util_top/transform_without_feedback.test.cc`, `vendor/stim/src/stim/util_top/circuit_flow_generators.test.cc`, and `vendor/stim/src/stim/util_top/has_flow.test.cc`.
- Add exact canonical-output tests for newly promoted transform cases.
- Add semantic tests that compare tableau action, detector error models, sampling distributions, or flow satisfaction before and after transforms.
- Add negative tests for unsupported feedback controls, unsupported repeat refolding, unsupported decomposition target shapes, invalid measurement-record rewrites, and excessive expansion.
- Add resource-boundary tests for nested large repeats and shift-only repeat folding.

Oracle rows:

- Supplement `pf2-circuit-decomposed`, `pf2-feedback-time-reverse`, and any transform row whose broad manifest-only coverage is replaced by executable evidence.
- Use exact-output rows only when pinned Stim text is stable.
- Use structural rows for semantic preservation or set-like outputs.

Benchmarks:

- Refresh existing report-only rows `pf2-circuit-flatten-repeat`, `pf2-circuit-without-noise`, `pf2-circuit-decompose-mpp-spp`, and `pf2-feedback-inline-batch` if implementation changes their hot paths.
- Keep `pf2-time-reverse-flow` synchronized with the scoped unitary `time_reversed_for_flows` runner, and extend or split the row when measurement-rich flow rewrites become active.
- Use schema-version-2 submeasurement thresholds if a row bundles multiple transform operations.

Acceptance criteria:

- Every promoted transform has exact or semantic parity tests against the owned subcases.
- Every unpromoted transform shape is documented and rejected precisely.
- No public transform path has unbounded repeat expansion.

## PFM3: Sweep-Conditioned Execution And Legal Gate Semantics

Objective: close remaining sweep-conditioned behavior and legal-gate execution gaps in the existing sampler, detector conversion, detection sampling, and analyzer surfaces.

Rows covered:

- Target kinds.
- Full semantic execution of every legal circuit operation.
- Gate semantic execution.
- Measurement-to-detection conversion.
- Broader sweep-conditioned simulator and analysis parity.

Tasks:

- Finish or explicitly reject frame-path sweep-conditioned detector sampling for the current public detection surface.
- Expand analyzer sweep behavior beyond the current no-op sweep-control subset only for selected exact subcases.
- Keep `m2d --sweep` and `--sweep_format` behavior synchronized with core converter behavior for every accepted input format.
- Classify legal gate execution support across sampler, converter, detection, and analyzer paths. The fixed-tableau gate contract is implemented for current sampler, detection-conversion, and analyzer surfaces; non-tableau legal operations remain active or explicitly rejected.
- Add precise errors for unsupported sweep target shapes, unsupported gate families, unsupported mixed feedback and sweep cases, and unsupported public output formats.
- Preserve streaming or documented caps for public inputs and outputs.

Tests:

- Port owned cases from `vendor/stim/src/stim/simulators/measurements_to_detection_events.test.cc`, `vendor/stim/src/stim/simulators/frame_simulator.test.cc`, `vendor/stim/src/stim/simulators/error_analyzer.test.cc`, `vendor/stim/src/stim/cmd/command_detect.test.cc`, and `vendor/stim/src/stim/cmd/command_m2d.test.cc`.
- Add sweep-record tests for `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64` wherever accepted.
- Add semantic tests comparing sweep-conditioned circuits to explicit small-circuit expansions.
- Add omitted-sweep default tests, width-mismatch tests, invalid-record-count tests, unsupported-format tests, unsupported-target-shape tests, and writer-error tests.
- Add gate execution tests that prove parser validation, sampler execution, detector conversion, and analyzer propagation do not drift.

Oracle rows:

- Supplement `pf3-sweep-m2d-detect`, `pf3-sweep-analyzer`, and `pf3-gate-semantic-execution` with executable rows for promoted subcases.
- CLI rows must prove stdout, stderr class, exit status, accepted flags, rejected flags, path behavior, and resource behavior.

Benchmarks:

- Keep or refresh report-only rows `pf3-m2d-sweep-b8`, `pf3-m2d-sweep-ptb64-input`, `pf3-detect-sweep-sampling`, and `pf3-analyze-errors-sweep`.
- Keep the implemented `pf3-gate-semantic-wide` row report-only unless a faithful pinned-Stim comparator is added.
- Classify CLI rows as `cli-baseline` only when pinned Stim exposes the same command shape.

Acceptance criteria:

- Every accepted sweep-conditioned path has core and public CLI evidence where the CLI exposes it.
- Every unsupported sweep shape fails before producing partial or misleading output.
- Parser-accepted gates have documented execution behavior for every implemented execution surface.

## PFM4: DEM APIs, Coordinates, Transforms, And Folded Traversal

Objective: finish active DEM Rust API gaps and remove avoidable expansion limits from DEM operations where practical.

Rows covered:

- DEM parser and canonical printer.
- DEM detector shifts, observables, coordinates, and counts.
- DEM flattening and large repeat traversal.
- DEM construction and mutation.
- DEM introspection.
- DEM transforms.
- DEM analysis and shortest graphlike error, for traversal behavior shared with PFM6.
- Full DEM public API parity, excluding diagrams and Python ergonomics.

Tasks:

- Decide whether any non-Python DEM mutation ergonomics remain active beyond `Clone`, constructors, append helpers, and push helpers.
- Finish selected folded coordinate behavior for large, nested, and ambiguous overlapping repeats, or keep documented caps with exact rejection tests.
- Finish folded or capped traversal behavior for DEM sampler, graphlike search, hypergraph search, SAT/WCNF generation, matcher-adjacent operations, and analyzer-adjacent operations where PFM4 owns the resource boundary.
- Preserve tags, separators, detector shifts, coordinate shifts, logical observables, repeat structure, and probability rounding contracts across public transforms.
- Keep all-detector materialization APIs capped when they must materialize large maps, and point callers to selected lookup APIs.

Tests:

- Port owned DEM cases from `vendor/stim/src/stim/dem/detector_error_model.test.cc`, `vendor/stim/src/stim/dem/dem_instruction.test.cc`, and Python DEM tests as semantic-mining sources.
- Add exact canonical-output tests for `flattened`, `rounded`, `without_tags`, tags, separators, detector shifts, coordinate shifts, logical observables, and repeats.
- Add structural tests for selected-coordinate lookup, all-detector coordinate maps, final coordinate shifts, final detector shifts, detector counts, observable counts, and error counts through large repeats.
- Add negative tests for invalid probabilities, invalid separators, invalid coordinate values, invalid repeat counts, detector-shift overflow, high ids, unsupported transform shapes, and non-finite folded coordinate results.
- Add resource-boundary tests for huge repeats, nested repeats, ambiguous overlapping repeats, and every consumer that still expands within a cap.

Oracle rows:

- Supplement `pf4-dem-introspection-transforms`, `pf4-dem-coordinate-api`, and `pf4-dem-folded-traversal` with executable rows whenever a broad manifest-only row becomes owned evidence.
- Exact rows should cover stable `.dem` text outputs.
- Structural rows should cover folded traversal, caps, and resource behavior.

Benchmarks:

- Keep or refresh report-only rows `pf4-dem-flatten-repeat`, `pf4-dem-rounded`, `pf4-dem-coordinate-map`, `pf4-dem-folded-traversal`, `pf4-dem-folded-graphlike-traversal`, and `pf4-dem-sampler-folded-repeat`.
- Promote only faithful direct-match rows with repeated stable evidence.
- Record measurement work units for detector count, detector coordinate lookup, flattened instruction count, and sampled or searched model size.

Acceptance criteria:

- DEM public APIs use typed detector ids, observable ids, coordinates, probabilities, repeat counts, and domain errors.
- Every public DEM transform and traversal consumer has folded behavior, bounded behavior, or precise rejection behavior.
- Any remaining cap is visible in the checklist and covered by tests.

## PFM5: Detector Utilities And Measurement-Rich Flows

Objective: finish active utility APIs for detecting regions, missing detectors, flow solving, and flow-dependent transform integration.

Rows covered:

- Detector-analysis utility APIs.
- Flows.
- Circuit transforms, for flow-aware transforms.
- Gate validation flags and categories, only when flow execution or transform integration reveals a metadata-contract drift.

Tasks:

- Extend `circuit_detecting_regions` for selected Clifford gates, target shapes, tick windows, detector filtering, multi-detector regions, anticommutation behavior, gauge behavior, and repeat traversal.
- Extend `missing_detectors` for selected generated honeycomb and toric suffix cases, plus any remaining MPP, pair-measurement, observable, gauge, and row-reduction cases that are not already implemented. The pinned honeycomb and toric global-stabilizer suffix cases are implemented; broader generated-code suffix analysis remains active.
- Implement measurement-rich `Flow`, `has_flow`, `has_all_flows`, `flow_generators`, `solve_for_flow_measurements`, flow multiplication, included observables, measurement indices, and failure diagnostics for the selected Rust scope. The `M`/`MX`/`MY`, `R`/`RX`/`RY`, `MR`/`MRX`/`MRY`, `MXX`/`MYY`/`MZZ`, nonconstant and constant single-instruction `MPP`, composed measurement-rich instruction sequences without ordinary unitary mixing, `MPAD`, scoped measurement-record feedback, promoted heralded-noise MPP `circuit_flow_generators`, and pinned Stim empty, `MX`, idle-extra-qubit, and repetition-code `solve_for_flow_measurements` examples are implemented; broader all-operation composed measurement-rich generators, broader heralded-noise generator synthesis, full generator-table measurement solving, and richer diagnostics remain active.
- Integrate measurement-rich flow semantics with the currently scoped unitary `time_reversed_for_flows` bridge, flow-aware decomposition checks, and feedback transforms while keeping the resolved gate-flow metadata contract synchronized.
- Add precise errors for unpromoted utility families.

Tests:

- Port owned cases from `vendor/stim/src/stim/util_top/circuit_to_detecting_regions.test.cc`, `vendor/stim/src/stim/util_top/missing_detectors.test.cc`, `vendor/stim/src/stim/stabilizers/flow.test.cc`, `vendor/stim/src/stim/util_top/circuit_flow_generators.test.cc`, and `vendor/stim/src/stim/util_top/has_flow.test.cc`.
- Add positive and negative tests for each promoted detecting-region gate, target shape, filter mode, multi-detector case, gauge case, and repeat case.
- Add positive and negative tests for generated-code missing-detector suffixes, MPP products, observables, row-reduction behavior, and unknown-input behavior.
- Add flow tests for measurement records, observables, multiplication, validation, generator solving, solve-for-measurements, sign behavior, and diagnostic quality.
- Add transform-integration tests proving flow-aware transforms preserve or intentionally rewrite flow data.

Oracle rows:

- Supplement `pf5-detecting-regions-extended`, `pf5-missing-detectors-extended`, and `pf5-measurement-rich-flows`.
- Use structural comparators for set-like results or ordering-insensitive outputs.
- Keep Python binding ergonomics out of the oracle claim.

Benchmarks:

- Keep or refresh `pf5-detecting-regions-repeat`, `pf5-missing-detectors-mpp`, `pf5-missing-detectors-generated-code`, `pf5-has-all-flows-batch`, `pf5-flow-generators-measurement-rich`, and `pf5-flow-solve-measurement-rich`.
- Keep rows report-only unless faithful Stim comparison and repeated stable ratios exist.

Acceptance criteria:

- Every promoted utility subfamily has positive, negative, and resource-boundary tests.
- Every unpromoted utility family fails closed or is logged as under-specified.
- Measurement-rich flows include measurement-record and observable cases in both success and failure paths.

## PFM6: Analyzer, Search, Sparse Reverse Tracking, And Active Matched-Error Values

Objective: close analyzer and logical-error search gaps without taking on full ErrorMatcher provenance or `stim explain_errors`.

Rows covered:

- Circuit-to-DEM analysis.
- `analyze_errors --decompose_errors` and related flags.
- DEM analysis and shortest graphlike error.
- Shortest graphlike and hypergraph logical-error search.
- Sparse reverse detector-frame tracking.
- Error explanation value objects only where active analyzer or search paths need them.

Tasks:

- Extend `circuit_to_detector_error_model` for selected generated circuits, loop folding, gauge detectors, approximate disjoint errors, decomposition options, remnant-edge blocking, and ignored decomposition failures.
- Extend graphlike, hypergraph, shortest-error, SAT, and WCNF behavior for selected generated-circuit and direct DEM cases.
- Add ordering-insensitive structural comparators for search outputs where exact target order is not stable.
- Improve sparse reverse detector-frame tracking for optimized loop folding and all-unitary fuzz coverage where that affects analyzer or search correctness. The supported-Clifford unitary-repeat folding subset is implemented; broader all-unitary fuzzing and analyzer/search-specific consumption remain active.
- Harden matched-error value objects only where active analyzer/search outputs require them.
- Keep full stack-frame provenance, heralded matching, repeat-contained noise provenance, and `explain_errors` CLI deferred.

Tests:

- Port owned cases from `vendor/stim/src/stim/simulators/error_analyzer.test.cc`, `vendor/stim/src/stim/simulators/error_matcher.test.cc`, `vendor/stim/src/stim/simulators/matched_error.test.cc`, `vendor/stim/src/stim/search/graphlike/algo.test.cc`, `vendor/stim/src/stim/search/hyper/algo.test.cc`, `vendor/stim/src/stim/search/sat/wcnf.test.cc`, and `vendor/stim/src/stim/util_top/circuit_to_dem.test.cc`.
- Add exact `.dem` output tests for deterministic analyzer cases.
- Add structural tests for generated circuits, loop folding, gauge detectors, approximate disjoint errors, decomposition options, ignored failures, graphlike search, hypergraph search, SAT/WCNF encoding, and shortest-error results.
- Add generated or fuzz tests for all-unitary sparse reverse tracking, repeated loops, detectors with coordinates, observables, and decomposed noise.
- Add negative tests for unsupported analyzer options, invalid decomposition behavior, excessive repeat expansion, and unsupported provenance requests.

Oracle rows:

- Supplement `pf6-analyzer-generated-looping`, `pf6-search-generated`, and `pf6-sparse-rev-tracker`.
- Keep `pf6-analyzer-generated-qec-rust` as evidence only for the generated-QEC subset it names.
- Use exact `.dem` comparators where output order is stable and structural comparators otherwise.

Benchmarks:

- Keep or refresh `pf6-analyze-errors-generated-surface`.
- Keep or refresh `pf6-graphlike-search-generated` and `pf6-hypergraph-search-generated`, which have report-only runner coverage for the promoted generated rotated-surface-code search subset.
- Implement `pf6-error-decomp-loop-folded` when its subcases are implemented; keep the implemented `pf6-sparse-rev-frame-loop` row report-only unless a faithful pinned-Stim comparator is added.
- Use schema-version-2 submeasurement thresholds for bundled analyzer or search rows.
- Promote only faithful pinned-Stim rows with repeated stable evidence.

Acceptance criteria:

- Analyzer and search outputs match pinned Stim for exact owned cases and satisfy structural comparators for ordering-insensitive cases.
- Loop folding is proven by tests and benchmark evidence, not only by small-output equality.
- Deferred provenance and CLI explanation surfaces stay outside completion claims.

## PFM7: Visible CLI Parity For `m2d`, `analyze_errors`, And Legacy Dispatch

Objective: finish active command-line gaps for `stab m2d`, `stab analyze_errors`, and accepted legacy aliases.

Rows covered:

- `stim m2d`.
- `stim analyze_errors`.
- Legacy top-level command flags.
- CLI binary rollup, for selected command behavior.
- Measurement-to-detection conversion, for public command behavior.

Tasks:

- Finish `stab m2d` parity for selected `--sweep`, `--sweep_format`, `--ran_without_feedback`, `--skip_reference_sample`, `--append_observables`, `--obs_out`, `--obs_out_format`, input formats, output formats, path errors, writer errors, stdout behavior, stderr class, exit status, and resource boundaries.
- Finish `stab analyze_errors` parity for selected decomposition flags, gauge behavior, approximate disjoint behavior, fold-loop behavior, input paths, output paths, stdout behavior, stderr class, exit status, and malformed input behavior.
- Finish accepted legacy alias behavior for `--gen`, `--convert`, `--sample`, `--detect`, `--m2d`, and `--analyze_errors`.
- Keep deprecated `--detector_hypergraph` rejected, absent from help topics, and excluded from this plan.
- Keep `diagram`, `explain_errors`, and `repl` commands deferred and fail closed.

Tests:

- Port owned cases from `vendor/stim/src/stim/cmd/command_m2d.test.cc`, `vendor/stim/src/stim/cmd/command_analyze_errors.test.cc`, `vendor/stim/src/stim/main_namespaced.test.cc`, and selected `vendor/stim/doc/usage_command_line.md` examples.
- Add exact oracle rows for accepted command shapes that have a faithful pinned Stim CLI comparator.
- Add Stab CLI tests for explicit rejections, invalid paths, nonexistent input files, unwritable output files, writer failures, malformed inputs, invalid formats, invalid observable side-output formats, unsupported `ptb64` output, feedback-inlining failures, and large input resource behavior.
- Add tests proving accepted aliases dispatch to the same command implementation and multiple legacy modes conflict.
- Add tests proving `--detector_hypergraph` remains unsupported.

Oracle rows:

- Supplement `pf7-m2d-cli-parity`, `pf7-analyze-errors-cli-parity`, and `pf7-legacy-dispatch-parity`.
- Exact rows must prove stdout, stderr class, exit status, accepted flags, rejected flags, path behavior, and resource behavior.
- Stab-only explicit rejections must have Stab CLI tests or oracle rows even when pinned Stim accepts a deprecated behavior Stab intentionally excludes.

Benchmarks:

- Keep or refresh `pf7-cli-m2d-sweep-b8`, `pf7-cli-m2d-feedback-inline`, `pf7-cli-analyze-errors-generated`, `pf7-cli-analyze-errors-decompose`, and `pf7-cli-legacy-dispatch-startup`.
- Promote only faithful pinned-Stim CLI rows with stable repeated evidence into the 1.25x threshold file.
- Keep startup and rejection-only rows report-only unless they become documented operational performance contracts.

Acceptance criteria:

- CLI behavior is proven by CLI tests or oracle rows, not only core tests.
- Public command paths stream or enforce documented caps.
- Help, README, checklist, roadmap, oracle manifest, benchmark manifest, and progress reports agree on the supported command surface.

## PFM8: Evidence, Benchmark Gate, Audit, Review, And Rollup Closure

Objective: turn child milestone evidence into durable acceptance evidence and update rollup rows without overstating parity.

Rows covered:

- Rust core library equivalent for core Stim semantics.
- CLI binary.
- `.stim`, `.dem`, and result-format compatibility.
- Full semantic execution of every legal circuit operation.
- Highest-priority remaining feature gaps.
- Any active child row completed by PFM1 through PFM7.

Tasks:

- For each completed child milestone, write or update a progress or completion report with implemented subcases, rejected subcases, deferred subcases, tests, oracle rows, benchmark rows, audit outcome, review outcome, and exact verification commands.
- Run milestone-audit for each completed milestone and fix implementation, test, benchmark, and documentation findings.
- Run full-code-review for each completed milestone and fix findings; when using Codex, spawn GPT-5.5/xhigh subagents during review work.
- Update `docs/stab-feature-checklist.md`, `docs/plans/partial-feature-inventory.md`, `docs/plans/rust-stim-drop-in-rewrite.md`, `docs/plans/stim-test-porting-plan.md`, oracle metadata, benchmark metadata, threshold files, waivers, profiler notes, and user-facing docs where behavior changed.
- Promote benchmark rows into `benchmarks/m12-primary-thresholds.json` only after faithful comparability, repeated stable evidence, profiler notes, and schema-version-2 threshold coverage exist.
- Keep report-only, contract-representative, proxy, tiny, no-ratio, and no-faithful-baseline rows out of the primary threshold file unless a source-owned waiver says otherwise.
- Update rollup checklist rows only after every active child row is implemented or explicitly deferred with a named reason.

Tests:

- `cargo test -p stab-bench --quiet`
- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::run --implemented-only`
- `just bench::smoke`
- Every milestone-specific test listed in PFM1 through PFM7.

Benchmarks:

- `just bench::baseline --primary --out target/benchmarks/non-deferred-partials-primary-baseline`
- `just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --baseline target/benchmarks/non-deferred-partials-primary-baseline/baseline.json --report target/benchmarks/non-deferred-partials-primary-compare`
- `just bench::primary-regression --baseline target/benchmarks/non-deferred-partials-primary-baseline/baseline.json --report target/benchmarks/non-deferred-partials-primary-regression`
- `just bench::primary-memory-regression --baseline target/benchmarks/non-deferred-partials-primary-baseline/baseline.json`

Acceptance criteria:

- Every completed child milestone has fresh evidence from current `HEAD` or an explicitly recorded local-modification state.
- No completion report cites exploratory probes, stale local reports, informal waivers, or broad upstream test files as authoritative evidence.
- Every remaining partial row either has a concrete active owner or is partial only because of an explicitly deferred surface.
- Rollup rows do not imply Python, JS/WASM, diagram, ecosystem, or simulator-product parity.

## Final Verification

Before claiming the whole plan complete, run:

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings
cargo test --workspace --quiet
just oracle::run --implemented-only
just bench::smoke
just maintenance::pre-commit
```

If a milestone changes benchmark gates, also run the PFM8 primary benchmark commands from current `HEAD`.
If a milestone changes public CLI behavior, include targeted CLI tests and oracle rows in the final evidence.
If a milestone changes public Rust APIs, update Rust docs or matching project docs in the same change set.

## Stop Conditions

Stop and write a `docs/plans/milestone-spec-gaps.md` entry when:

- A promoted subcase requires an excluded surface.
- A whole upstream file is still being treated as acceptance criteria.
- A public CLI behavior cannot define accepted flags, rejected flags, input formats, output formats, stdout behavior, stderr class, exit status, path handling, and resource behavior.
- A benchmark row cannot be assigned a comparability class.
- A performance claim would require stale reports, unrecorded local modifications, missing profiler notes, or informal waivers.
- A public parser, converter, sampler, analyzer, transformer, search, or writer path has neither streaming behavior nor a documented cap.
- A checklist update would need to hide a known limitation to mark a row done.
