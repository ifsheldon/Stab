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
- Generated Python or JS/WASM API documentation, Python stubs, and generated feature/status matrix tooling.
  The Rust API documentation workflow is implemented by `just docs::api` and `just docs::api-check`; expanding beyond that workflow needs a separate documentation source-of-truth plan.

If a subcase in this plan turns out to require an excluded surface, stop and log the under-specification in `docs/plans/milestone-spec-gaps.md` instead of silently widening scope.

## Covered Partial Rows

| Plan milestone | Checklist rows covered | Notes |
| --- | --- | --- |
| PFM0 | Rollup rows and future checklist drift | Reconcile rows that are partial mostly because deferred Python or product surfaces are absent, and split any remaining active Rust subcases before implementation. The selected PF1 circuit Rust API rows for programmatic mutation, core introspection, circuit coordinate queries, and reference samples or determined measurements are already closed by `pf1-circuit-rust-api`; the selected PF1 DEM construction and mutation row is closed by `pf1-dem-rust-api`; current Rust and CLI streaming DEM sampling plus detector sampling rows are closed by [pfm0-sampling-streaming-evidence-lock.md](pfm0-sampling-streaming-evidence-lock.md); matrix, state-vector, and arbitrary-unitary conversion parity is closed as deferred with scoped M6/M12 Rust semantic evidence by [pfm0-matrix-simulator-deferral-evidence-lock.md](pfm0-matrix-simulator-deferral-evidence-lock.md); public graph/vector and other simulator-product rows remain deferred. |
| PFM1 | Gate semantic execution, full semantic execution of every legal circuit operation, flows | Keep metadata and execution-support contracts synchronized after the current Rust gate metadata surface closed under `pf1-gate-metadata-api`. Finish remaining execution and flow-integration gaps without reopening Python `GateData` shape. |
| PFM2 | Repeat handling, circuit transforms, measurement-to-detection conversion, full circuit transform API parity, full feedback-inlining transform parity | Finish flow-aware transforms, feedback-loop decisions, repeat traversal behavior, and transform resource boundaries. |
| PFM3 | Target kinds, gate semantic execution, measurement-to-detection conversion, broader sweep-conditioned simulator and analysis parity | Finish or precisely reject remaining sweep-conditioned execution and analyzer subcases. |
| PFM4 | DEM parser and canonical printer evidence lock, DEM detector shifts, DEM introspection, DEM transforms, DEM flattening and large repeat traversal, full DEM public API parity | Finish DEM API gaps and folded or capped traversal behavior for selected consumers. DEM parser and canonical printer status is closed by the PFM0 evidence lock and should be reopened only if parser or printer behavior changes; DEM construction and mutation for the current Rust API surface is already closed by `pf1-dem-rust-api`; Python ergonomics remain deferred. |
| PFM5 | Detector-analysis utility APIs, flows, circuit transforms, gate validation flags and categories | Finish detecting regions, missing detectors, measurement-rich flow solving, and flow-driven transform integration. |
| PFM6 | Circuit-to-DEM analysis, `analyze_errors --decompose_errors`, DEM analysis and shortest graphlike error, shortest graphlike and hypergraph search, sparse reverse detector-frame tracking, active matched-error value objects | Finish analyzer/search/sparse-tracker gaps without taking on full ErrorMatcher provenance or `explain_errors` CLI. |
| PFM7 | `stim m2d`, `stim analyze_errors`, legacy top-level command flags, CLI binary | Finish visible CLI parity for selected commands and accepted legacy aliases, with `--detector_hypergraph` remaining excluded. The selected CLI binary rollup is now closed; reopen only for newly selected command behavior. |
| PFM8 | Rust core library equivalent, `.stim`/`.dem`/result-format compatibility, full semantic execution, highest-priority remaining feature gaps, and CLI binary regression checks | Audit, review, benchmark, documentation, and rollup-status closure after child milestones have evidence. |

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
- Classification-heavy active rows such as DEM introspection, rollup rows, and any future checklist drift introduced after child milestones land.
- Closed PF1 circuit Rust API rows only for evidence-lock regression checks, not for new implementation work.
- Closed PF1 DEM construction and mutation rows only for evidence-lock regression checks, not for new implementation work.
- Rollup rows that depend on active child rows.
- Deferred simulator-product rows only to confirm that the deferral reason remains explicit, not to reopen `TableauSimulator` or `FlipSimulator` work.

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

Current evidence:

- `docs/plans/pfm0-current-evidence-lock-report.md` records the current PFM0 classification pass, metadata checks, and next active implementation candidates.

## PFM1: Gate Metadata And Execution-Support Contract

Objective: keep parser acceptance, metadata availability, and execution support impossible to confuse while active execution and flow-integration gaps close.

Rows covered:

- Gate semantic execution.
- Full semantic execution of every legal circuit operation, for gate-table and execution-support bookkeeping.
- Flows, for execution and transform integration beyond gate-level flow metadata.

Tasks:

- Treat current Rust gate metadata accessors, unsupported-accessor errors, and metadata-column support-contract synchronization as closed by `pf1-gate-metadata-api`.
- Keep the resolved decision that measurement-rich and variable-target `GateData.flows` metadata belongs in `Gate::flows`, while sampler, detector-conversion, analyzer, and full circuit flow execution support remain separate milestone surfaces.
- Keep `SPP` and `SPP_DAG` parser, decomposition metadata, sampler execution, detection-conversion execution, and analyzer execution behavior synchronized. The sampler, detection-conversion planner, and detector-frame paths execute supported Hermitian products via decomposition lowering, while analyzer state and gauge-tracker paths execute supported Hermitian products via unsigned Pauli-product propagation.
- Update `docs/plans/rpf1-gate-execution-support-contract.md`, the checklist, and milestone reports whenever execution support changes so parser acceptance remains separate from execution support.

Tests:

- Keep the table-driven metadata-column support-contract test current when metadata columns change, and add execution-column checks when an implementation slice changes execution support.
- Port owned metadata cases from `vendor/stim/src/stim/gates/gates.test.cc`, `vendor/stim/src/stim/gates/gates_test.py`, and gate data tests as semantic sources.
- Add execution-boundary tests proving parser-accepted but unsupported execution gates fail with precise domain errors in the surfaces where they remain unsupported, and positive decomposition-equivalence tests whenever sampler or detector-conversion support is promoted.

Oracle rows:

- Keep implemented rows such as `pf1-gate-metadata-api`, `pf1-gate-decomposition-metadata`, and `pf3-gate-semantic-wide-rust` current if public API names, metadata behavior, or execution behavior change.

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

- Finish flow-semantic checks for `Circuit::decomposed` that depend on PFM5 measurement-rich flows. Selected MPP and pair-measurement decomposition flow-generator preservation is implemented; broader decomposition flow semantics remain active when new measurement-rich flow families are promoted.
- Promote `Circuit::inverse_qec` beyond the unitary subset only for selected QEC inverse cases with exact owned subcases. The selected noiseless plain no-flow reset-measure-detector target-list shape, selected exact two-to-one `R`/`CX`/`M` detector-flow shape, selected exact `m_det` two-detector shape, selected exact noiseless `MPP` plus one-detector all-record identity-parity shape, selected exact noisy `MZZ` detector-flow shape, selected exact observable Pauli include packet, selected noisy `M`/`MX`/`MY` measurement-only reversal, selected noisy `MR`/`MRX`/`MRY` measure-reset-only reversal, selected exact noisy measure-reset detector-flow packet, and selected measure-reset pass-through target-list shape are implemented for their selected detector counts, including pinned `r_m_det`, pinned `two_to_one`, pinned `m_det`, pinned `mpp`, pinned `mzz`, pinned `obs_include_pauli`, pinned `noisy_m`, pinned `noisy_mr`, pinned `noisy_mr_det`, pinned `pass_through`, multi-target record remapping where selected, sparse detector subsets for the no-flow packet, duplicate detector-record parity where selected, empty-detector behavior where selected, and tag or coordinate preservation where applicable, while prior-measurement detector refs, broader noisy measure-reset detector-flow, detector-flow rewrites with interleaved operations beyond the exact two-to-one packet, selected `m_det` packet, selected MPP identity-parity packet, selected noisy `MZZ` packet, and selected noisy measure-reset detector-flow packet, broader observable-aware QEC inverse rewrites beyond the selected observable Pauli include packet, feedback, repeats, and multi-instruction QEC inverse behavior remain active.
- Promote `time_reversed_for_flows` beyond the current unitary Rust subset for the measurement-rich flow cases selected by PFM5. The selected single-instruction measurement-rich subset for one noiseless plain unique-target `M`, `MX`, `MY`, `MXX`, `MYY`, or `MZZ` instruction group, selected multi-record measurement-ordering examples, selected plain `R`, `RX`, and `RY` reset-to-measurement conversion over one or more unique qubit targets, selected single-target `M`, `MX`, and `MY` measurement-to-reset conversion, one noiseless `MR`, `MRX`, or `MRY` instruction over one or more unique qubit targets including inverted result targets, and the selected single-record `MZZ` plus plain-qubit unitary suffix packet matching pinned `flow_through_mzz_h_cx_s` are implemented with pinned `M`, `R`, and `MZZ` examples plus source-owned basis, reset, measure-reset, and unitary-suffix coverage; duplicate reset-only and duplicate measure-reset behavior remain explicitly fail-closed pending the compatibility decisions logged in `docs/plans/milestone-spec-gaps.md`, and detectors, feedback, noise, repeats, and broader multi-instruction QEC inverse behavior remain active.
- Extend feedback loop handling beyond the selected `XCZ`/`YCZ` measurement-record feedback, bounded pinned repeat-loop, and nested bounded-repeat detector-parity cases only when the exact repeat-contained feedback subcases, resource bounds, comparator, tests, and benchmark evidence are specified.
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

- Finish or explicitly reject frame-path sweep-conditioned detector sampling for the current public detection surface. Selected frame-path Pauli-observable `detect` sampling with sweep-controlled `CX` and `CY` qubit targets, sweep-controlled `CZ`, `CZ` bit/bit no-op groups, and `XCZ` or `YCZ` qubit/sweep groups now uses omitted all-false sweep bits, and measurement-record `XCZ` or `YCZ` target feedback is accepted as X or Y feedback equivalent to `CX` or `CY` feedback. Pinned Stim v1.16.0 does not expose `stim detect --sweep`, so typed `detect` sweep files are not a CLI parity target; Python detector-sampler sweep-bit APIs and future explicit Rust detector-sampler APIs remain deferred unless a later plan selects them.
- Expand analyzer sweep behavior beyond the original single no-op sweep-control subset only for selected exact subcases. The selected analyzer sweep-control matrix now covers `CX`, `CY`, `CZ`, `XCZ`, and `YCZ` no-op behavior, including `CZ` sweep/sweep, record/sweep, sweep/record, and record/record classical-only no-op groups, plus invalid controlled-Pauli target-position rejections; broader analyzer sweep-shape parity remains active.
- Keep `m2d --sweep` and `--sweep_format` behavior synchronized with core converter behavior for every accepted input format. The current public CLI matrix covers `01`, `b8`, `r8`, `hits`, `dets`, and input-only `ptb64` sweep records under `pf3-m2d-sweep-format-matrix-cli`.
- Close sampler-backed target-order drift for Stim-parsed sweep targets: `CX q sweep[k]` and `CY q sweep[k]` must reject in reference sampling, detection conversion, non-frame detection sampling validation, and `stab m2d --sweep`, while sweep-first `CX` or `CY` and both-order `CZ` sweep/qubit groups must remain accepted.
- Classify legal gate execution support across sampler, converter, detection, and analyzer paths. The fixed-tableau gate contract is implemented for current sampler, detection-conversion, and analyzer surfaces; non-tableau legal operations remain active or explicitly rejected.
- Add precise errors for unsupported sweep target shapes, unsupported gate families, unsupported mixed feedback and sweep cases, and unsupported public output formats.
- Preserve streaming or documented caps for public inputs and outputs.

Tests:

- Port owned cases from `vendor/stim/src/stim/simulators/measurements_to_detection_events.test.cc`, `vendor/stim/src/stim/simulators/frame_simulator.test.cc`, `vendor/stim/src/stim/simulators/error_analyzer.test.cc`, `vendor/stim/src/stim/cmd/command_detect.test.cc`, and `vendor/stim/src/stim/cmd/command_m2d.test.cc`.
- Add sweep-record tests for `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64` wherever accepted. The accepted public `m2d --sweep_format` matrix is implemented by `m2d_accepts_sweep_records_in_all_text_and_byte_formats` and `m2d_accepts_sweep_records_in_ptb64_format`.
- Add semantic tests comparing sweep-conditioned circuits to explicit small-circuit expansions.
- Add target-order tests for sampler-backed sweep Pauli operations, including both rejected `CX q sweep[k]` and `CY q sweep[k]` cases and accepted sweep-first `CX` or `CY` plus both-order `CZ` cases.
- Add omitted-sweep default tests, width-mismatch tests, invalid-record-count tests, unsupported-format tests, unsupported-target-shape tests, and writer-error tests.
- Add gate execution tests that prove parser validation, sampler execution, detector conversion, and analyzer propagation do not drift.

Oracle rows:

- Supplement `pf3-sweep-m2d-detect`, `pf3-sweep-analyzer`, and `pf3-gate-semantic-execution` with executable rows for promoted subcases. `pf3-m2d-sweep-format-matrix-cli` is the executable row for the accepted public `m2d --sweep_format` input matrix.
- `pf3-sampler-sweep-target-order-rust` is the executable structural row for sampler-backed `CX q sweep[k]` and `CY q sweep[k]` rejection plus accepted neighboring target orders.
- CLI rows must prove stdout, stderr class, exit status, accepted flags, rejected flags, path behavior, and resource behavior.

Benchmarks:

- Keep or refresh report-only rows `pf3-m2d-sweep-b8`, `pf3-m2d-sweep-ptb64-input`, `pf3-detect-sweep-sampling`, and `pf3-analyze-errors-sweep`.
- Keep the implemented `pf3-gate-semantic-wide` row report-only unless a faithful pinned-Stim comparator is added.
- Classify CLI rows as `cli-baseline` only when pinned Stim exposes the same command shape.
- Do not add a benchmark row for pure target-order rejection slices unless implementation changes a hot validation path.

Acceptance criteria:

- Every accepted sweep-conditioned path has core and public CLI evidence where the CLI exposes it.
- Every unsupported sweep shape fails before producing partial or misleading output.
- The sampler-backed `CX` or `CY` sweep target-order boundary matches pinned Stim execution behavior without narrowing flow-generator no-op semantics.
- Parser-accepted gates have documented execution behavior for every implemented execution surface.

## PFM4: DEM APIs, Coordinates, Transforms, And Folded Traversal

Objective: finish active DEM Rust API gaps and remove avoidable expansion limits from DEM operations where practical.

Rows covered:

- DEM parser and canonical printer evidence lock, with no active parser/printer implementation work unless behavior changes.
- DEM detector shifts, observables, coordinates, and counts.
- DEM flattening and large repeat traversal.
- DEM introspection.
- DEM transforms.
- DEM analysis and shortest graphlike error, for traversal behavior shared with PFM6.
- Full DEM public API parity, excluding diagrams, Python ergonomics, and the already closed current Rust construction and mutation helper subset.

Tasks:

- Treat current Rust DEM construction and mutation helpers as closed by `pf1-dem-rust-api`; do not add more non-Python mutation ergonomics unless a concrete active Rust consumer needs them and the plan is updated first.
- Finish selected folded coordinate behavior for large, nested, and ambiguous overlapping repeats, or keep documented caps with exact rejection tests. Selected large non-flat sparse coordinate holes now use actual declared-detector bounds to avoid impossible candidate iteration scans while preserving documented caps for genuinely ambiguous cases. The pinned Stim generated surface-code coordinate comparison is now counted only through the PFM6 bounded mixed-top-level analyzer fallback for prefix, repeat, and tail circuits under `fold_loops=true`; true folded generated-loop output remains active PFM6 work.
- Finish folded or capped traversal behavior for DEM sampler, graphlike search, hypergraph search, SAT/WCNF generation, matcher-adjacent operations, and analyzer-adjacent operations where PFM4 owns the resource boundary. Graphlike and hypergraph search skip zero-probability repeated bodies, fold selected flat detector-touching and detectorless logical-only zero-shift repeated error bodies, skip selected flat no-target zero-shift repeated error bodies, fold selected flat zero-detector-shift `shift_detectors 0` repeated bodies, fold selected flat annotation-bearing repeated bodies with `detector` or standalone `logical_observable` declarations, fold selected nested zero-shift repeated bodies, fold selected mixed zero-probability plus active zero-shift repeated error bodies, sparsely index selected high-detector direct DEM search graphs, and keep caps for non-selected shifted active repeats plus shifted nested, non-flat, numeric-target, separator-only, or broader non-annotation mixed-instruction active repeats; ErrorMatcher filter DEM traversal folds selected flat detector-touching zero-shift repeated keys while shifted filter DEM repeats and repeat-contained circuit noise remain capped or rejected; weighted SAT/WCNF omits zero-probability error variables, skips repeated zero-probability bodies before flattening, folds selected flat and nested zero-shift repeat bodies by concrete MAP parity cost, and keeps a dense SAT target cap for shifted active errors; unweighted SAT folds selected flat and nested zero-shift repeat bodies structurally, including zero-probability and no-target mechanisms, while high-index dense-target, shifted, non-flat, or other non-selected structural repeats remain capped, with dedicated high-observable dense-cap evidence for selected zero-probability structural repeats; DEM sampler direct detection-event output skips zero-probability repeated bodies, folds deterministic zero-shift repeats by parity, folds selected single-stochastic zero-shift repeats by odd-parity probability, folds selected flat stochastic zero-shift repeats by per-error odd-parity probability, and folds selected nested zero-shift stochastic repeats by recursive effective-error parity.
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

- Keep or refresh report-only rows `pf4-dem-flatten-repeat`, `pf4-dem-rounded`, `pf4-dem-coordinate-map`, `pf4-dem-folded-traversal`, `pf4-dem-folded-graphlike-traversal`, `pf4-dem-hypergraph-logical-repeat`, `pf4-dem-hypergraph-no-target-repeat`, `pf4-dem-search-zero-shift-repeat`, `pf4-dem-search-annotation-repeat`, `pf4-dem-search-mixed-zero-probability-repeat`, `pf4-dem-search-nested-repeat`, `pf4-dem-sat-flat-repeat-fold`, and `pf4-dem-sampler-folded-repeat`.
- Promote only faithful direct-match rows with repeated stable evidence.
- Record measurement work units for detector count, detector coordinate lookup, flattened instruction count, skipped zero-probability error occurrences, skipped no-target error occurrences, folded zero-detector-shift target-error occurrences, folded annotated target-error occurrences, folded active target-error occurrences for mixed zero-probability search repeats, folded nested target-error occurrences, folded deterministic error occurrences, folded stochastic error occurrences, folded flat stochastic error occurrences, folded nested stochastic error occurrences, folded SAT error occurrences, folded nested SAT error occurrences, and sampled or searched model size.

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

- Extend `circuit_detecting_regions` for selected Clifford gates, target shapes, tick windows, detector or logical-observable filtering, multi-detector regions, anticommutation behavior, gauge behavior, and repeat traversal. The bounded repeat traversal, detector and logical-observable `DemTarget` filters, dense-capped default-like all-target/all-tick helpers, pinned `MX`, `MZZ`, and tagged ordinary-noise detecting-region examples, generated repetition-code all-target/all-tick selection with selected exact D0, D6, and L0 regions, selected generated rotated and unrotated surface-code all-target/all-tick helper counts plus exact D0, D4, and L0 regions, the full single-qubit Clifford gate set with plain qubit targets, fixed two-qubit tableau-backed Clifford gates with plain qubit-pair targets, selected measurement-record feedback placements for `CX`, `CY`, `CZ`, `XCZ`, and `YCZ`, selected gate-order-valid sweep-controlled Pauli sign-only no-op groups for `CX`, `CY`, `CZ`, `XCZ`, and `YCZ` plus selected `CZ` classical-only bit-bit groups, explicit fail-closed validation for non-`CZ` sweep/sweep, record/sweep, and record/record groups, ordinary non-record-producing noise no-op traversal, inverted targets for promoted measurement and reset-measurement families, `MPAD` measurement pads, `MPP` Pauli-product measurement targets, `SPP` and `SPP_DAG` unitary Pauli-product target shapes, `HERALDED_ERASE` and `HERALDED_PAULI_CHANNEL_1` record-producing noise with plain qubit targets, ignored anticommutation mode, selected measurement-gauge ignored-mode behavior, and product-measurement gauge-cancellation behavior are implemented; broader target shapes outside the promoted positive set and source-owned fail-closed set, broader generated-code regions beyond the promoted repetition-code and selected rotated and unrotated surface-code cases, and broader gauge behavior remain active.
- Extend `missing_detectors` for selected generated honeycomb and toric suffix cases, plus any remaining MPP, pair-measurement, observable, gauge, Clifford propagation, repeat traversal, and row-reduction cases that are not already implemented. The pinned honeycomb and toric global-stabilizer suffix cases, tableau-backed single-qubit and fixed two-qubit Clifford propagation with plain qubit target groups, `SPP` and `SPP_DAG` unitary Pauli-product analysis through decomposition, bounded repeat traversal with explicit expansion caps, and selected folded final-repeat traversal for covered deterministic measurement loops with flat or bounded nested local bodies are implemented; broader generated-code suffix analysis and broader folded large-repeat traversal remain active.
- Keep the basic `Flow` object surface closed by `coverage-stabilizers-flow`: parsing, canonicalized measurement indices, included observables, display, ordering, and multiplication are already covered and should be reopened only if the Rust API changes. Implement measurement-rich `has_flow`, `has_all_flows`, `flow_generators`, `solve_for_flow_measurements`, diagnostics where selected, signed sampled checking if selected, folded traversal, and transform integration for the selected Rust scope. The `M`/`MX`/`MY`, `R`/`RX`/`RY`, `MR`/`MRX`/`MRY` including inverted result targets, `MXX`/`MYY`/`MZZ`, Python multi-target `M`/`MX`/`MYY`/`MPP`, nonconstant and constant single-instruction `MPP`, pinned variable-target `SPP` and `SPP_DAG` unitary generator examples, selected unitary-mixed composed measurement-rich instruction sequences, selected all-operation annotation and ordinary-noise no-op traversal, selected composed `SPP` and `SPP_DAG` unitary decomposition, the pinned generated all-operations fixture, bounded repeat-contained measurement-rich instruction sequences, `MPAD`, scoped measurement-record feedback, selected gate-order-valid sweep-controlled Pauli sign-only no-op groups, promoted heralded-noise MPP `circuit_flow_generators`, unsigned `has_flow` and `has_all_flows` Rust helpers for the promoted checker subset, an additive unsigned diagnostic checker for output mismatches, input mismatches, out-of-range measurement records, and unsupported-circuit reasons, scoped signed sampled flow checking for unitary, measurement-record, observable-record, observable-Pauli, and inverted-observable cases, and pinned Stim empty, `MX`, measured-idle, multi-target measurement and `MPP`, fewer-measurements heuristic, and repetition-code `solve_for_flow_measurements` examples are implemented; broader all-operation composed measurement-rich generators beyond the promoted pinned generated all-operations fixture, no-op, tableau, measurement-record feedback, selected gate-order-valid sweep-controlled Pauli sign-only no-op groups, heralded-record, and `SPP` or `SPP_DAG` decomposition subcases, broader heralded-noise generator synthesis, folded repeat traversal beyond the current flow-row and materialized flattened-operation caps, full generator-table measurement solving, and broader solver or generator diagnostics remain active.
- Integrate measurement-rich flow semantics with the currently scoped unitary `time_reversed_for_flows` bridge, flow-aware decomposition checks, and feedback transforms while keeping the resolved gate-flow metadata contract synchronized. The selected measurement-rich time-reversal bridge is implemented for one noiseless plain unique-target measurement instruction group with selected measurement-ordering evidence, selected plain reset-to-measurement conversion over one or more unique qubit targets, selected single-target measurement-to-reset conversion, one noiseless measure-reset instruction over one or more unique qubit targets including inverted result targets, and the selected single-record `MZZ` plus plain-qubit unitary suffix packet matching pinned `flow_through_mzz_h_cx_s`; broader transform integration remains active.
- Add precise errors for unpromoted utility families.

Tests:

- Port owned cases from `vendor/stim/src/stim/util_top/circuit_to_detecting_regions.test.cc`, `vendor/stim/src/stim/gen/gen_surface_code.test.cc`, `vendor/stim/src/stim/util_top/missing_detectors.test.cc`, `vendor/stim/src/stim/stabilizers/flow.test.cc`, `vendor/stim/src/stim/util_top/circuit_flow_generators.test.cc`, `vendor/stim/src/stim/util_top/circuit_flow_generators_test.py`, and `vendor/stim/src/stim/util_top/has_flow.test.cc`.
- Add positive and negative tests for each promoted detecting-region gate, target shape, filter mode, multi-detector case, gauge case, and repeat case.
- Add positive and negative tests for generated-code missing-detector suffixes, MPP products, observables, row-reduction behavior, Clifford propagation, `SPP` and `SPP_DAG` decomposition parity, repeat traversal, and unknown-input behavior.
- Keep the existing `coverage-stabilizers-flow` tests for `Flow` object parsing, measurement records, observables, multiplication, validation, and sign behavior current when the object API changes; add or refresh flow tests for promoted `has_flow`, generator solving, solve-for-measurements, failure paths, signed sampled behavior if selected, transform integration, and diagnostic quality. The current unsigned diagnostic checker is covered by `pf5-has-flow-diagnostics-rust`; future diagnostic rows should be added only when solver, generator, signed sampled, or transform diagnostics are selected.
- Add transform-integration tests proving flow-aware transforms preserve or intentionally rewrite flow data.

Oracle rows:

- Supplement `pf5-detecting-regions-extended`, `pf5-missing-detectors-extended`, and `pf5-measurement-rich-flows`.
- Keep `pf5-detecting-regions-generated-unrotated-surface-rust` synchronized with the selected unrotated surface-code D0, D4, and L0 exact-output slice.
- Use structural comparators for set-like results or ordering-insensitive outputs.
- Keep Python binding ergonomics out of the oracle claim.

Benchmarks:

- Keep or refresh `pf5-detecting-regions-repeat`, `pf5-detecting-regions-targets`, `pf5-detecting-regions-clifford`, `pf5-detecting-regions-generated-repetition`, `pf5-detecting-regions-generated-surface`, `pf5-missing-detectors-mpp`, `pf5-missing-detectors-generated-code`, `pf5-has-all-flows-batch`, `pf5-flow-generators-measurement-rich`, `pf5-flow-generators-measurement-python`, `pf5-flow-solve-measurement-rich`, and `pf5-flow-solve-measurement-python`.
- Do not add a separate unrotated surface-code detecting-region benchmark row unless a future workload exercises a distinct performance path; the current unrotated slice is exact-output test and oracle evidence only.
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

- Extend `circuit_to_detector_error_model` for selected generated circuits, loop folding, gauge detectors, approximate disjoint errors, decomposition options, remnant-edge blocking, and ignored decomposition failures. The selected prefix, repeat, and tail detector-chain shape under `fold_loops=true` now has true compact folded output after candidate validation against a measurement-record-lookback-sized non-folded expansion, including the selected large repeat-count resource case. Delayed measurement-record dependencies that the selected compact fold cannot prove fall back to the bounded non-folded analyzer. The selected generated surface-code prefix, repeat, and tail coordinate shape still uses a bounded mixed-top-level fallback that reuses the capped non-folded analyzer, which is enough to prove the pinned Stim coordinate comparison but does not claim true folded output for broader generated-loop families. The selected upstream guard for folded observable dependencies that cross iterations without including every loop-carried measurement is implemented as a nondeterministic-observable rejection, not folded-output support.
- Extend graphlike, hypergraph, shortest-error, SAT, and WCNF behavior for selected generated-circuit and direct DEM cases.
  The selected direct DEM graphlike and hypergraph exact-output subset is implemented by `pf6-search-direct-dem-graphlike-rust` and `pf6-search-direct-dem-hypergraph-rust`, the selected direct DEM sparse high-detector resource subset is implemented by `pf6-search-sparse-high-detectors-graphlike-rust` and `pf6-search-sparse-high-detectors-hypergraph-rust`, the selected high-observable analyzer-to-search subset is implemented by `pf6-search-many-observables-graphlike-rust` and `pf6-search-many-observables-hypergraph-rust`, the selected generated-QEC graphlike and hypergraph search subset is implemented by `pf6-search-generated-qec-rust`, and the selected generated-QEC SAT/WCNF structural subset is implemented by `pf6-search-generated-sat-wcnf-rust`.
- Add or extend ordering-insensitive structural comparators for search outputs where exact target order is not stable. The selected generated-QEC graphlike and hypergraph search rows now include canonical target-set uniqueness, deterministic error rows, zero detector parity, and exact `L0` observable parity; broader generated families and tie-sensitive target-set comparators remain active.
- Improve sparse reverse detector-frame tracking for optimized loop folding and analyzer or search correctness. The supported-Clifford unitary-repeat folding subset is implemented for the full single-qubit Clifford gate set and fixed two-qubit tableau-backed Clifford gates with plain qubit-pair targets, with deterministic generated repeat tests covering nested repeats, multi-target single-qubit instructions, multi-pair two-qubit instructions, and no-fold traversal comparisons. The shifted-copy measurement/detector repeat subset is implemented by `pf6-sparse-rev-shifted-repeat-rust`, covering record and detector offset comparison, shifted target application, small unrolled equivalence, public unsigned-flow consumption, and a trillion-iteration period skip. The unsigned sparse-tracker path also supports `SPP` and `SPP_DAG` product propagation for public unsigned-flow checking. Analyzer/search-specific consumption and broader variable-target unitary semantics outside this unsigned tracker path remain active.
- Harden matched-error value objects only where active analyzer/search outputs require them. The selected `ExplainedError` and `CircuitErrorLocation` canonicalization slice is implemented by `pf6-matched-error-canonicalize-rust`; broader hardening remains active only when new analyzer or search outputs require it.
- Keep full stack-frame provenance, heralded matching, repeat-contained noise provenance, and `explain_errors` CLI deferred.

Tests:

- Port owned cases from `vendor/stim/src/stim/simulators/error_analyzer.test.cc`, `vendor/stim/src/stim/simulators/error_matcher.test.cc`, `vendor/stim/src/stim/simulators/matched_error.test.cc`, `vendor/stim/src/stim/search/graphlike/algo.test.cc`, `vendor/stim/src/stim/search/hyper/algo.test.cc`, `vendor/stim/src/stim/search/sat/wcnf.test.cc`, and `vendor/stim/src/stim/util_top/circuit_to_dem.test.cc`.
- Add exact `.dem` output tests for deterministic analyzer cases.
- Add structural tests for generated circuits, loop folding, gauge detectors, approximate disjoint errors, decomposition options, ignored failures, graphlike search, hypergraph search, SAT/WCNF encoding, and shortest-error results.
- Add generated or property-style tests for sparse reverse tracking when new supported unitary families, repeated loops, detectors with coordinates, observables, decomposed noise, analyzer consumption, or search consumption are promoted.
- Add negative tests for unsupported analyzer options, invalid decomposition behavior, excessive repeat expansion, and unsupported provenance requests.

Oracle rows:

- Supplement `pf6-analyzer-generated-looping`, `pf6-search-generated`, and `pf6-sparse-rev-tracker`.
- Keep `pf6-analyzer-generated-qec-rust` as evidence only for the generated-QEC subset it names.
- Keep `pf6-analyzer-prefix-repeat-tail-folding-rust` as evidence only for the selected true folded prefix/repeat/tail detector-chain output, tail-error output, and large repeat-count resource case it names.
- Keep `pf6-error-decomp-loop-folded-rust` as evidence only for the selected loop-folded decomposition and remnant-edge blocking subset it names.
- Keep `pf6-analyzer-mixed-top-level-fallback-rust` and `pf6-analyzer-generated-fold-loop-fallback-rust` as evidence only for unsafe or still-unsupported bounded non-folded fallback, analyzer-budget cap preservation, selected folded-error non-masking guard, and selected generated surface-code coordinate comparison they name.
- Keep `pf6-analyzer-folded-observable-guard-rust` as evidence only for the selected nondeterministic-observable rejection where loop-carried observable dependencies cross folded iterations.
- Keep `pf6-search-direct-dem-graphlike-rust`, `pf6-search-direct-dem-hypergraph-rust`, `pf6-search-sparse-high-detectors-graphlike-rust`, and `pf6-search-sparse-high-detectors-hypergraph-rust` as evidence only for the selected direct DEM graphlike and hypergraph rejection, distance, canonical-ordering, bounded hyper-error, and sparse high-detector resource cases they name.
- Keep `pf6-search-many-observables-graphlike-rust` and `pf6-search-many-observables-hypergraph-rust` as evidence only for the selected high-observable analyzer-to-search `many_observables` cases that prove `L1200` parity survives graphlike and hypergraph search output.
- Keep `pf6-sparse-rev-unitary-repeat-rust` and `pf6-sparse-rev-shifted-repeat-rust` as evidence only for the selected unitary-repeat and shifted measurement/detector repeat sparse-tracker folding cases they name.
- Keep `pf6-matched-error-canonicalize-rust` as evidence only for the selected matched-error value-object canonicalization behavior it names.
- Use exact `.dem` comparators where output order is stable and structural comparators otherwise.

Benchmarks:

- Keep or refresh `pf6-analyze-errors-generated-surface`.
- Keep or refresh `pf6-graphlike-search-generated`, `pf6-hypergraph-search-generated`, and `pf6-generated-sat-wcnf`, which have report-only runner coverage for the promoted generated rotated-surface-code search and SAT/WCNF subsets.
- Keep `pf6-error-decomp-loop-folded` synchronized with the promoted repeated composite-error loop-folded decomposition subset, and extend or split it if broader decomposition families become active; keep the implemented `pf6-sparse-rev-frame-loop` row report-only unless a faithful pinned-Stim comparator is added.
- Do not add a benchmark for matched-error canonicalization unless a future analyzer or search surface puts it on a measured throughput path.
- Use schema-version-2 submeasurement thresholds for bundled analyzer or search rows.
- Promote only faithful pinned-Stim rows with repeated stable evidence.

Acceptance criteria:

- Analyzer and search outputs match pinned Stim for exact owned cases and satisfy structural comparators for ordering-insensitive cases.
- Loop folding is proven by tests, candidate validation, resource-boundary evidence, and benchmark evidence where the promoted slice is a throughput path, not only by small-output equality.
- Deferred provenance and CLI explanation surfaces stay outside completion claims.

## PFM7: Visible CLI Parity For `m2d`, `analyze_errors`, And Legacy Dispatch

Objective: finish active command-line gaps for `stab m2d`, `stab analyze_errors`, and accepted legacy aliases.

Rows covered:

- `stim m2d`.
- `stim analyze_errors`.
- Legacy top-level command flags.
- CLI binary rollup, closed for selected command behavior.
- Measurement-to-detection conversion, for public command behavior.

Tasks:

- Finish `stab m2d` parity for selected `--sweep`, `--sweep_format`, `--ran_without_feedback`, `--skip_reference_sample`, `--append_observables`, `--obs_out`, `--obs_out_format`, input formats, output formats, writer errors, stdout behavior, stderr class, exit status, and resource boundaries. The selected `m2d` CLI closure is implemented by `pf7-m2d-cli-parity`, with supporting path-IO evidence in `pf7-m2d-path-io-rust`, command-contract evidence in `pf7-m2d-command-contract-rust`, selected `XCZ`/`YCZ` feedback coverage under `--ran_without_feedback`, and existing M9 sweep, feedback, format, and resource coverage; keep adding rows only for newly selected command shapes, path failure modes, format failures, or resource failures.
- Finish `stab analyze_errors` parity for selected decomposition flags, gauge behavior, approximate disjoint behavior, fold-loop behavior, input paths, output paths, stdout behavior, stderr class, exit status, and malformed input behavior. The selected flag and malformed-input subset is implemented by `pf7-analyze-errors-flags-rust`, and the selected `analyze_errors` CLI closure is implemented by `pf7-analyze-errors-cli-parity`; keep adding rows only for newly selected flags, malformed inputs, or analyzer failure modes.
- Accepted legacy alias behavior for `--gen`, `--convert`, `--sample`, `--detect`, `--m2d`, and `--analyze_errors` is implemented by `pf7-legacy-dispatch-accepted-rust`, and the selected legacy-dispatch closure is implemented by `pf7-legacy-dispatch-parity`; keep adding rows only for newly selected legacy spellings or failure modes.
- Keep deprecated `--detector_hypergraph` rejected, absent from help topics, and excluded from this plan.
- Keep `diagram`, `explain_errors`, and `repl` commands deferred and fail closed.

Tests:

- Port owned cases from `vendor/stim/src/stim/cmd/command_m2d.test.cc`, `vendor/stim/src/stim/cmd/command_analyze_errors.test.cc`, `vendor/stim/src/stim/main_namespaced.test.cc`, and selected `vendor/stim/doc/usage_command_line.md` examples.
- Add exact oracle rows for accepted command shapes that have a faithful pinned Stim CLI comparator.
- Add Stab CLI tests for explicit rejections, invalid paths, nonexistent input files, unwritable output files, writer failures, malformed inputs, invalid formats, invalid observable side-output formats, unsupported `ptb64` output, feedback-inlining failures, and large input resource behavior; the selected `m2d` CLI closure is covered by `pf7-m2d-cli-parity` with focused path-IO and command-contract evidence in `pf7-m2d-path-io-rust` and `pf7-m2d-command-contract-rust`, and the selected `analyze_errors` CLI closure is covered by `pf7-analyze-errors-cli-parity` with focused flag evidence in `pf7-analyze-errors-flags-rust`.
- Maintain `pf7-legacy-dispatch-parity` as the selected legacy-dispatch closure, `pf7-legacy-dispatch-accepted-rust` for accepted aliases that dispatch to the same command implementation, and `pf7-legacy-dispatch-conflicts-rust` for multiple legacy mode conflicts.
- Add tests proving `--detector_hypergraph` remains unsupported.

Oracle rows:

- Keep `pf7-m2d-cli-parity` implemented as the selected `m2d` CLI closure with focused path-IO and command-contract evidence in `pf7-m2d-path-io-rust` and `pf7-m2d-command-contract-rust`. Keep `pf7-analyze-errors-cli-parity` implemented as the selected `analyze_errors` CLI closure with focused flag evidence in `pf7-analyze-errors-flags-rust`, and keep `pf7-legacy-dispatch-parity` implemented as the selected legacy-dispatch closure with accepted aliases covered by `pf7-legacy-dispatch-accepted-rust`.
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
- CLI binary status synchronization after selected command closure.
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
