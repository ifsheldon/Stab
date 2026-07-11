# Non-Deferred Partial Feature Milestones

## Summary

This plan covers every feature row that is marked `Partial` in `docs/stab-feature-checklist.md` and still has non-deferred Rust or CLI work.
It excludes rows whose remaining work is only Python bindings, JavaScript/WASM, diagrams, ecosystem integrations, public simulator products, C++ header compatibility, exact random-stream parity, or deprecated `--detector_hypergraph` support.

This is the active planning document for finishing the remaining partial feature surfaces and resolving the eight open under-specification entries recorded in `docs/plans/milestone-spec-gaps.md`.
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

## Historical PFM Dependency Map

The original PFM0 through PFM8 dependency relationships remain useful background:

1. Run PFM0 before each new wave if the checklist, inventory, or roadmap has changed.
2. Run PFM1 and PFM5 before measurement-rich flow-dependent PFM2 work, because measurement-rich `time_reversed_for_flows` and flow-aware decomposition checks need measurement-rich flow semantics.
3. Run PFM3 before PFM7 when CLI `m2d` or `detect` work depends on core sweep behavior.
4. Run PFM4 before PFM6 when analyzer or search work depends on DEM folded traversal behavior.
5. Run PFM8 only after one or more implementation milestones have fresh source-owned evidence.

Milestones may be implemented in smaller slices, but every slice must name the checklist rows, oracle rows, benchmark rows, deferred edges, and done criteria it owns.
The active execution order is PFM-B0, PFM-B2 contract groundwork, PFM-B3, the PFM-B4 flow foundation, PFM-B1, PFM-B5, PFM-B2 final generated coverage, and PFM-B6.

## Remaining Blocker Closure Program

The current blockers are planning and architecture blockers, not permission to keep adding isolated fixture-shaped branches.
This program replaces the previous instruction to choose an arbitrary next subcase from each broad checklist row.
It gives every open entry in `docs/plans/milestone-spec-gaps.md` one finite disposition: implement a general capability, close the entry from existing source-owned evidence, or explicitly defer a product surface already excluded by this plan.

### Blocker Decisions

| Blocker | Decision | Closure path |
| --- | --- | --- |
| PFM2 broader QEC inverse and measurement-rich transforms | Implement | Finish the pinned non-binding transform corpus and replace packet-specific flow rewriting with one reverse-flow engine for the selected Rust API. |
| PFM3 analyzer sweep shapes | Evidence close | The current matrix already exceeds the only pinned C++ analyzer sweep case. Do not invent additional shapes without a failing pinned oracle or a newly selected public API. |
| PFM3 legal non-tableau execution | Implement | Build an exhaustive gate-by-surface support contract and generated semantic tests instead of promoting gates one example at a time. |
| PFM4 DEM folded traversal and coordinates | Implement | Introduce a shared checked folded traversal abstraction, migrate bounded-result consumers, and retain explicit caps only where the requested result itself is materialized or potentially exponential. |
| PFM5 detecting regions | Evidence close | Audit and lock the pinned `simple` and start-of-circuit anticommutation cases plus the already promoted generated, gauge, target, and resource evidence. Broader invented families are not active blockers. |
| PFM5 missing detectors | Evidence close | Audit and lock the pinned `circuit`, `big_case_honeycomb_code`, and `toric_code_global_stabilizer_product` cases plus current row-reduction and repeat evidence. Broader invented families are not active blockers. |
| PFM5 flow generators, solving, diagnostics, and transform integration | Implement | Replace bounded exhaustive solving and circuit-shape dispatch with GF(2) elimination over a shared stabilizer-flow engine, then close the named C++ and Python semantic corpus. |
| PFM6 analyzer, search, sparse tracker, and active matched errors | Implement | Replace fixture-signature loop folding with generic state-cycle folding, close named search and SAT families, and integrate the shared sparse tracker into active consumers. Full ErrorMatcher provenance remains deferred. |

An evidence-close milestone may change status and documentation but may not add speculative behavior merely to make a broad sentence look fuller.
An implementation milestone may preserve a documented cap when output size or algorithmic complexity is inherently proportional to the expanded request, but it may not materialize repeated input merely for internal inspection.

### PFM-B0: Freeze The Closure Ledger

Objective: turn the blocker decisions into an executable, subcase-level evidence ledger before production code changes.

Status: Complete as of 2026-07-10. The original 124-case source ledger, selector validation, exact oracle evidence signatures, typed benchmark runner, threshold, and comparability classifications, milestone audit, and full code review are recorded in `docs/plans/pfm-b0-blocker-ledger-progress-report.md`; PFM-B2 separated deterministic MPP, anti-Hermitian MPP rejection, deterministic MPAD, stochastic MPP, and stochastic MPAD provenance and added explicit identity-noise and control-flow owners, so the current ledger has 130 cases without changing the blocker set.

Tasks:

- Add a schema-versioned `docs/plans/blocker-closure-ledger.json` with one record per owned upstream subcase, not one record per upstream file, and render its human-readable summary in the PFM8 progress report.
- Give each row a stable id, owning blocker, public or internal surface, Stim source and test name, expected comparator, implementation status, Rust test filter, oracle row, benchmark row or no-benchmark rationale, resource contract, and disposition.
- Split multi-example upstream tests such as `circuit_flow_generators.various`, `missing_detectors.circuit`, and Python `test_inv_circuit` into stable named subcases before counting them.
- Mark already implemented subcases as evidence verification work instead of scheduling duplicate implementation.
- Record selector reuse explicitly in the generated summary; shared selectors are allowed only as PFM-B0 evidence-splitting debt and do not satisfy the later owner milestone's independently selectable evidence criterion.
- Freeze every promoted supporting oracle row required by an evidence-close decision, including public CLI, generated-code, target-shape, MPAD, and repeat/resource evidence that is not the primary pinned subcase row.
- Mark Python binding shape, file-like Python behavior, diagrams, public simulator products, exact randomness, and full ErrorMatcher provenance as deferred without using those deferrals to hide Rust semantic gaps.
- Reject any proposed row whose expected behavior cannot be obtained from pinned Stim v1.16.0, a stated Stab hardening decision, or a precise semantic invariant.

Tests and operational checks:

- `cargo test -p stab-oracle fixtures --quiet`
- `just oracle::blockers --check-selectors`
- `just oracle::list`
- `just oracle::matrix --check`
- `just bench::list`
- Add a `stab-oracle` validation test that rejects a blocker-ledger record without a comparator, resource contract, benchmark disposition, and either a planned selector for planned work or an executable selector for implemented and evidence-closed work.

Benchmarks:

- No timings are required in PFM-B0.
- Existing rows must be mapped to exact ledger entries and must keep their current comparability classes.

Acceptance criteria:

- All eight open spec-gap entries have an implementation or evidence-close owner in this program.
- No owned row cites a whole upstream file as sufficient completion evidence.
- No blocker remains open solely because the checklist says `broader`, `full`, or `every other consumer`.
- Shared broad test filters remain visible with exact owning case ids and cannot be mistaken for independently selectable completion evidence.

### PFM-B1: General Reverse-Flow And QEC Transform Closure

Implementation checkpoint, 2026-07-11: all nineteen ledger cases are implemented with distinct exact Cargo selectors and the packet-specific measurement-rich dispatch is replaced by one `ReverseFlowTransition` and sparse reverse-tracker engine. Review findings are fixed and rechecked with no remaining P0 through P2 blocker: high-index unitary validation has a tableau budget and sparse fallback, empty nested repeats and checker batches skip unnecessary work, returned flows validate in one batch, width-mismatched idle flow qubits use sparse validation, reversal-only record aliases reject like pinned Stim while ordinary unsigned checking retains XOR cancellation, observable effects combine before collapse checks, exact goldens re-record through a pinned C++ helper with path and SHA-256 bindings, truncated or link-routed evidence fails closed, source-owned and child-produced inputs are bounded, live side-output limits terminate process groups, scratch directories clean up through RAII, ignored tests cannot satisfy evidence, generated-surface workloads are repeat-free compact-source points, MPAD and repeat allocation properties have executable incremental-slope bounds, and sparse-index allocation has an additive-delta bound. The implementation commit and clean committed-HEAD allocation reports remain before completion; see `docs/plans/pfm-b1-reverse-flow-progress-report.md`.

Objective: close the PFM2 blocker with a general reverse-flow implementation for the selected Rust transform APIs, not more exact-circuit recognizers.

Owned pinned subcases:

- Audit or implement C++ `circuit_inverse_qec.anticommute`, `flow_reverse`, `flow_through_mzz`, and `flow_past_end_of_circuit` as separately selectable Rust tests.
- Audit or implement Python `test_inv_circuit` examples, `test_inv_circuit_surface_code`, `test_more_flow_qubits_than_circuit_qubits`, `test_measurement_ordering`, `test_measurement_ordering_2`, `test_measurement_ordering_3`, `test_feedback`, and `test_obs_include_paulis` at the Rust semantic level without claiming Python binding parity.
- Preserve the already promoted exact packets for reset, measurement, measure-reset, MPP, MZZ, MPAD, noisy measurement, noisy measure-reset, observable include, `flow_flip`, and the selected unitary suffix.
- Add a bounded MPAD matrix covering constant values `0` and `1`, record-only flow terms, observable-only flow terms, mixed record and observable terms, duplicate observable-id parity, and one interleaved supported Clifford on either side. Obtain expected results from pinned Stim before implementation.

Implementation requirements:

- Represent reverse propagation as one typed flow state containing Pauli input, Pauli output, measurement parity, and observable parity.
- Reuse the shared Clifford and sparse reverse-tracker operations for unitary propagation.
- Handle measurement, reset, measure-reset, pair measurement, MPP, MPAD, detector, and observable annotations through gate-family handlers, not whole-circuit pattern matching.
- Keep feedback fail-closed where pinned Stim rejects it.
- Support repeat blocks through a generic unitary cycle summary or a documented bounded traversal; do not add a handler keyed to a fixture's exact period.
- Validate every returned flow with the unsigned checker when unsigned semantics apply, and compare detecting regions for the generated rotated surface-code reversal.

Tests:

- Exact canonical-circuit and exact-flow tests for deterministic pinned examples.
- Structural flow-set comparison for generated circuits where flow order is not contractual.
- Property tests generating small supported circuits and satisfiable flows, reversing them twice where the operation family is involutive, and checking returned flows against the checker.
- Negative tests for anticommutation, unsatisfied flows, feedback, duplicate targets under the locked hardening policy, out-of-range records, non-finite probabilities, and repeat-budget overflow.
- Resource tests for large unitary repeats and bounded rejection of non-foldable measurement-rich repeats.

Oracle rows:

- Split `pf2-time-reverse-flow-measurement-rust` so the named C++ and Python subcases are independently selectable.
- Add a structural generated-surface reversal row and a focused MPAD flow-matrix row.
- Keep exact-output comparators for stable pinned text and structural comparators for detecting-region sets.

Benchmarks:

- Preserve the historical measurement-rich corpus key and add repeat-free generated-surface size, MPAD flow-count, unitary-repeat count/body, and nonempty-flow sparse high-qubit submeasurement matrices with explicit compact-source, flow, or transform work units plus peak live bytes.
- Keep the row report-only unless a faithful in-process pinned Stim comparator is added.
- Require the generated-surface matrix to expose state/source-size growth, the repeat matrix to distinguish count from compact body/state size, and the low-versus-million qubit-index matrix to reject allocation proportional to untouched intermediate IDs.

Acceptance criteria:

- Every owned pinned subcase has direct executable evidence.
- Production transform code contains gate-family logic and reusable state transitions, not fixture text, exact detector counts, or exact operation-sequence signatures.
- The PFM2 spec-gap entry is resolved for the selected Rust transform API, and remaining Python or export surfaces are explicitly deferred instead of leaving the row generically partial.

### PFM-B2: Gate-By-Surface Semantic Contract

Objective: resolve both PFM3 entries by closing unsupported ambiguity across parser, measurement sampler, reference sampler, detection converter, detector frame, detection sampler, error analyzer, and flow-generator surfaces.

Status: Contract groundwork is complete as of 2026-07-10 and recorded in `docs/plans/pfm-b2-gate-surface-contract-groundwork-report.md`. Final generated semantic execution, statistical evidence, oracle shards, and any production dispatch fixes remain pending after the shared B3, B4, B1, and B5 foundations stabilize.

Tasks:

- Add one source-owned gate-by-surface contract generated from canonical gate metadata.
- Classify every canonical gate on each relevant surface as `execute`, `semantic_noop`, `annotation`, `lower_then_execute`, `unsupported_shape`, or `not_applicable`.
- Permit `unsupported_shape` only for an invalid target-role combination or a shape that has a named exclusion outside the selected Rust or CLI surface; it cannot be used to claim semantic completion for a legal shape inside the selected surface.
- Record accepted target-role patterns separately from gate names, including plain qubits, inverted measurement targets, Hermitian and anti-Hermitian Pauli products, combiners, measurement records, sweep bits, constants, and detector or observable declarations whose IDs remain governed by their canonical argument rules.
- Map parser-accepted targets back to machine-readable target patterns and generate tests from the contract so parser acceptance cannot be mistaken for execution support.
- Cover fixed-tableau gates, reset and measurement families, pair measurements, MPP, MPAD, SPP and SPP_DAG, Pauli noise, Pauli channels, depolarization, correlated-error blocks, heralded noise, annotations, controlled-Pauli feedback, and sweep-controlled groups.
- Give each `pfm3-contract-*` ledger case an exact machine-readable set of all eight contract surfaces; reject missing, duplicate, or unknown surface values.
- Give every one of the nineteen canonical semantic families at least one machine-readable ledger owner, including exact no-op identity-noise and structural control-flow shards; reject missing or duplicate family declarations.
- Validate the ledger's typed surface and semantic-family names against the canonical core contract so the two schemas cannot drift independently.
- Treat `FlowGenerator` as the PFM-B2 flow surface; flow generation, checking evidence, and solving were owned and closed by PFM-B4, while reverse-flow and QEC transform integration remain owned by PFM-B1.
- Evidence-close analyzer sweep behavior at the current selected matrix because pinned Stim supplies no additional concrete analyzer case.
- Require a new failing pinned oracle, a public API expansion, or an explicit compatibility decision before reopening analyzer sweep shapes.

Tests:

- Contract completeness test requiring every canonical gate and relevant surface to have exactly one classification.
- Accepted-target classification tests covering mixed classical-control groups plus Hermitian and anti-Hermitian Pauli-product groups.
- Generated positive tests for each executable or no-op class and generated negative tests for each unsupported target role.
- Exact deterministic comparisons for reference samples, detection conversion, and analyzer DEM output.
- Statistical comparisons for stochastic MPP, MPAD, Pauli channels, depolarization, correlated errors, and heralded noise with source-owned shot counts, tolerances, and false-positive budgets.
- Cross-surface tests proving the same target-role pattern is accepted or rejected consistently wherever the contract says the surfaces share semantics.
- Sweep ordering tests for `CX`, `CY`, `CZ`, `XCZ`, and `YCZ`, including the current classical-only no-op matrix and omitted all-false sweep behavior.
- Maximum legal sweep-ID analyzer regression proving resource use is not proportional to sweep-index magnitude.
- Feature-gated allocation regression comparing `sweep[0]` with `sweep[16777215]`, allowing at most two additional allocation calls and 1,024 additional total or peak-live bytes for the maximum ID.

Oracle rows:

- Replace broad `pf3-gate-semantic-execution` wording with generated contract shards grouped by gate family.
- Keep deterministic MPP, anti-Hermitian MPP rejection, deterministic MPAD, stochastic MPP, and stochastic MPAD evidence in separate ledger cases with exact, error-class, or statistical comparators as appropriate.
- Keep `pf3-sweep-analyzer` as the complete selected analyzer sweep row and record that no unowned pinned subcase remains.
- Use statistical rows only for genuinely probabilistic behavior.

Benchmarks:

- Add no per-gate microbenchmarks.
- Keep representative sampler, converter, detector-frame, and analyzer rows for fixed-tableau, Pauli-product, and stochastic families.
- Extend `pf3-analyze-errors-sweep` with separate low and maximum sweep-ID submeasurements so `just bench::compare-allocations --only pf3-analyze-errors-sweep` records the resource evidence.
- Add one mixed-contract compile and execute row whenever final PFM-B2 changes a production compile or execution path to consult the contract; if production dispatch remains unchanged, retain the no-new-row disposition and cite the static-only diff. Classify a new row as `direct-match` only with a faithful pinned Stim workload.

Acceptance criteria:

- The contract has no `unknown`, `selected_example_only`, or implicit fallback state.
- Every parser-accepted canonical gate has explicit behavior on every relevant implemented surface.
- Every accepted target group maps to one or more declared target patterns, including a typed rejection for anti-Hermitian Pauli products on semantic surfaces.
- Every legal target-role shape inside the selected Rust or CLI surface is `execute`, `semantic_noop`, `annotation`, or `lower_then_execute`; any `unsupported_shape` record points to an invalid combination or an explicit exclusion that remains visible in status documentation.
- Analyzer sweep scope is closed from evidence, and legal non-tableau execution is closed from the exhaustive contract and generated tests.

### PFM-B3: Shared Folded DEM Traversal

Completion checkpoint, 2026-07-10: PFM-B3 is complete for the selected Rust surface. The shared traversal, seven consumer migrations, exact selectors, focused oracle rows, contract-only core benchmark, and materialization rationales are implemented. All milestone-audit and GPT-5.6/max review findings are fixed, and the final allocation report records committed `HEAD=4a984c26b39f6236fde5e3ff10cf0b42e8b155a2` with `local_modifications=false`, peak live allocation of 65,536 bytes, and zero resident delta. The non-blocking ledger-proxy and statistical-schema follow-ups are recorded in `docs/plans/milestone-spec-gaps.md` and do not reopen this milestone.

Objective: resolve the PFM4 blocker with one checked traversal model shared by bounded-result DEM consumers.

Implementation requirements:

- Introduce an internal folded visitor or cursor with checked scalar block summaries, detector offset, folded depth, and folded multiplicity. Coordinate vectors are opt-in state used only by coordinate APIs, repeat depth is a checked summary used by consumers that historically own a nesting cap, and unrelated consumers must not scan targets or allocate coordinate vectors.
- Support early termination and selected-detector or selected-instruction filtering without flattening preceding repeats.
- Expose repeat summaries to consumers that can combine repeated state, while allowing bounded expansion only when a consumer's requested output is itself expanded.
- Migrate counts, selected coordinate lookup, all-coordinate lookup, DEM sampler compilation, graphlike and hypergraph collection, SAT/WCNF collection, and ErrorMatcher filtering to the shared traversal. Keep `rounded` and tag stripping on direct compact recursive transforms because building an auxiliary traversal tree would violate their output-only allocation contract; document this deliberate non-migration.
- Keep `DetectorErrorModel::flattened` capped because its public result is materialized.
- Keep full coordinate-map output proportional to the number of returned detector ids. Pinned Stim returns every detector id below `count_detectors()`, including empty coordinate vectors, so the full map is inherently materialized and may remain capped; selected lookup must avoid scanning nonexistent sparse detector ids.
- Use checked arithmetic for repeat counts, detector shifts, coordinate shifts, target rebasing, and output-size estimates.
- Treat expanded detector-declaration cardinality as a thresholded capability: overflow means "above the bounded local-scan limit" and must not block algebraic selected lookup.
- Bound coordinate-only scalar work at 8,000,000 updates, use fallible coordinate-vector growth, and preserve coordinate-free count, sampler, search, SAT/WCNF, and matcher paths even when ignored coordinate accumulation would overflow.
- Skip empty and annotation-only neutral repeats before search, SAT/WCNF, and ErrorMatcher expansion-cap checks.

Tests:

- Table-driven models covering flat, nested, empty-body, single-iteration, zero-shift, nonzero detector-shift, coordinate-shift, sparse detector-id, annotation-only, logical-only, separator-bearing error, and mixed active and zero-probability repeat bodies.
- Run 96 deterministic Proptest cases with ChaCha seed `b3` repeated to 32 bytes, maximum generated depth 3, maximum recursive size 48, branch width 4, root width 6, repeat counts 1 through 3, detector shifts 0 through 2, coordinate widths 0 through 3, tags, annotations, zero and deterministic active errors, graphlike separator shapes, and detector or observable targets. Compare scalar summaries, full coordinates, compact transforms, deterministic sampler records, graphlike and hypergraph results, and ErrorMatcher filter results against explicitly unrolled DEMs. SAT/WCNF uses separate literal pinned-text assertions because semantically equivalent folded and expanded encodings need not use the same variable shape.
- Huge-repeat tests proving selected lookup, count, sampler compilation, and eligible search or SAT collection do not scale with repeat count.
- Negative tests for arithmetic overflow, excessive result cardinality, ambiguous coordinate declarations, excessive nesting, and consumers whose inherent output or search space exceeds documented caps.
- Visitor-error tests proving immediate stop and error preservation.

Oracle rows:

- Add one focused folded-traversal row per migrated consumer instead of a single broad DEM row.
- Use exact comparators for canonical DEM and WCNF text, structural comparators for search sets, seeded semantic comparators for sampler compilation, and absolute tolerance `1e-12` for fractional coordinates because algebraic folded multiplication and pinned Stim's sequential addition differ in floating-point rounding.

Benchmarks:

- Add `flat-equivalent`, `nested-large-repeat`, `sparse-selected-coordinate`, and `wide-coordinate-irrelevant` submeasurements for the shared visitor.
- Refresh consumer rows only when their traversal path changes.
- Record instructions or declarations visited, logical expanded work represented, peak live bytes, and sampled resident delta.
- Require huge-repeat eligible consumers to remain bounded in memory by model body plus consumer state.

Acceptance criteria:

- No bounded-result DEM consumer expands repeats merely to inspect them.
- Every remaining materializing consumer names an inherent output or complexity reason and has a tested cap.
- Folded and explicitly unrolled traversal agree on the defined 96-case deterministic generated corpus, while literal pinned WCNF, fractional-coordinate tolerance, statistical sampler, neutral-repeat, overflow, error-class, and resource regressions cover contracts that cannot use byte-identical materialization.

### PFM-B4: Detector Utilities And General Flow Solving

Completion checkpoint, 2026-07-11: all forty-nine PFM-B4 ledger cases have exact independently selectable evidence, the over-sixteen solver case is implemented, exhaustive measurement-subset fallback is removed, and generator construction plus sparse unsigned checking share typed reverse-transition classification. Successive milestone-audit and GPT-5.6/max findings covering correctness, evidence, resource behavior, compatibility, benchmark semantics, and documentation are closed. Implementation commit `0f47eee04eacec96ed4e03dd36a18f58b76a0afc` and both required clean allocation reports identify `local_modifications=false` with zero resident delta. PFM-B4 is complete; see `docs/plans/pfm-b4-detector-flow-progress-report.md`.

Objective: evidence-close the two already covered detector utilities and finish the real PFM5 flow-engine gap.

Detecting-region evidence closure:

- Lock C++ `circuit_to_detecting_regions.simple` and Python `test_detecting_regions_fails_on_anticommutations_at_start_of_circuit` as exact owned upstream subcases.
- Retain current source-owned tests for filters, ticks, generated repetition and surface codes, gauge handling, Clifford propagation, feedback, sweep no-ops, MPP, MPAD, SPP, heralded records, repeats, and resource limits.
- Do not invent broader generated-code or gauge families without a failing pinned case or a separately approved product requirement.

Missing-detector evidence closure:

- Lock C++ `missing_detectors.circuit`, `big_case_honeycomb_code`, and `toric_code_global_stabilizer_product` as independently selectable subcases.
- Retain current source-owned tests for row reduction, observables, MPAD, MPP, Clifford and SPP propagation, bounded repeats, and folded final repeats.
- Do not treat every possible generated code or stabilizer-rank pattern as an active blocker.

Flow implementation requirements:

- Drive `circuit_flow_generators`, unsigned checking, and `solve_for_flow_measurements` from shared stabilizer transitions instead of whole-circuit shape dispatch.
- Replace the bounded exhaustive measurement-selection fallback with GF(2) elimination over generator measurement signatures.
- Define deterministic pivot and tie-breaking rules from pinned Stim outputs so exact examples remain stable.
- Preserve sparse qubit ids, measurement-record parity, observable parity, repeat budgets, and typed diagnostics.
- Give `time_reversed_for_flows` the same transition implementation through PFM-B1 instead of duplicating measurement semantics.

Owned flow subcases:

- Split C++ `circuit_flow_generators.various` into stable per-example ids, then cover `all_operations`, `solve_for_flow_measurements.empty`, `simple`, and `rep_code`.
- Cover Python `test_solve_flow_measurements`, `test_solve_flow_generators_measurements_multi_target`, `test_solve_flow_measurements_multi_target`, and `test_solve_flow_measurements_fewer_measurements_heuristic` at the Rust semantic level.
- Keep the promoted signed and unsigned checker, diagnostics, repeat, sweep, feedback, heralded MPP, SPP, and MPAD cases.

Tests:

- Exact flow-list tests where pinned ordering is stable and structural span-equivalence tests where bases are non-unique.
- Property tests requiring every generated flow to satisfy the unsigned checker and every solved flow, after attaching the returned measurements, to match the requested input/output Pauli projection. Like Stim v1.16.0, `solve_for_flow_measurements` ignores measurement and observable terms already present on query flows; observable-aware validity belongs to the signed and unsigned checker tests.
- Rank-deficient, inconsistent, underdetermined, sparse-high-qubit, nonempty ignored query-term, and more-than-20-measurement solver cases, with duplicate measurement and observable parity retained in the `Flow` value-object and checker suites that actually consume those terms.
- Repeat and resource tests proving elimination is polynomial in the generator matrix dimensions and does not enumerate measurement subsets.
- Existing detecting-region and missing-detector exact-output, negative, and resource tests must remain green.

Oracle rows:

- Promote focused exact rows for the two detecting-region upstream tests and three missing-detector upstream tests.
- Split flow rows by generator, solver, checker, and transform integration; do not use one row to imply all four.

Benchmarks:

- Keep existing detecting-region and missing-detector rows report-only unless a faithful pinned comparator exists.
- Split flow solving into deterministic measurement-rich scrambled dense `32x64` and `128x256` Pauli bases carrying exact 7- and 24-singleton measurement-signature sets plus a `512x1024` high-qubit Pauli basis carrying an exact 12-singleton measurement-signature set and scrambled within exactly 32 sparse active qubits. Every workload circuit must contain one controlled-Pauli instruction that mixes a classical-feedback target group with a plain two-qubit target group, which is the shape that forced the pre-PFM-B4 generator path into the removed fallback; the 24-measurement medium case must therefore fail the former sixteen-measurement fallback cap. Use 17, 65, and 33 queries respectively; compose every query from exactly three generator rows with distinct singleton measurement signatures, and require every solution to have nonempty measurement parity. Time only end-to-end `solve_for_flow_measurements`, including generator construction and query reduction, plus black-box result consumption, while keeping deterministic fixture construction and all contract validation outside the timing sample. Report query-inclusive Pauli input bits and solved queries per second plus peak live bytes. Enforce at least 15% overall density for dense cases, at most 8% overall density for the sparse case, 15% through 85% density inside every active submatrix, and exact equality between the declared active-qubit set and measurement-bearing Pauli support. Pin literal production work values, exact singleton-signature sets, and the mixed controlled-Pauli shape, and execute production case construction in benchmark-harness tests so nominal dimensions cannot masquerade as meaningful work.
- Add a faithful direct comparator only if pinned Stim can execute the same generated circuit and requested-flow batch without Python binding overhead.

Acceptance criteria:

- Detecting regions and missing detectors close from exact evidence without speculative scope growth.
- Flow solving no longer has an exhaustive-subset size cliff.
- All named C++ and Python semantic subcases have executable evidence, and every produced flow passes the appropriate checker.

### PFM-B5: Generic Analyzer Loop Folding And Search Closure

Objective: resolve the PFM6 blocker by replacing fixture-shaped folding with a general finite-state loop summary and by closing a finite search corpus.

Analyzer implementation requirements:

- Define a loop-boundary analyzer state containing the sparse detector frame, measurement lookback dependencies, observable dependencies, coordinate shift, detector shift, and pending correlated-error state required for semantic recurrence.
- Detect transient and periodic states by canonical state identity, then compose the repeated DEM delta with checked arithmetic.
- Support prefix, nested repeat, and tail composition through the same summary API.
- Preserve Stim-compatible compact output for owned deterministic cases and use bounded candidate validation against unrolled execution during implementation and tests.
- Remove production branches that recognize exact period-8, period-127, or fixture-specific instruction signatures once the generic path covers them.
- Keep bounded fallback only for states that cannot be summarized, and prove the fallback cannot mask errors from a selected folded path.

Owned analyzer subcases:

- Port or lock exact slices from `ErrorAnalyzer.loop_folding`, `loop_folding_nested_loop`, `loop_folding_rep_code_circuit`, `multi_round_gauge_detectors_dont_grow`, `coordinate_tracking`, and `dont_fold_when_observable_dependencies_cross_iterations`.
- Retain the existing prefix/repeat/tail, huge odd observable, period-8, period-127, decomposition, remnant-edge, generated surface-coordinate fallback, and folded-observable guard evidence.

Search and SAT closure:

- Cover graphlike `no_error`, `distance_1`, `distance_2`, `distance_3`, `surface_code`, `repetition_code`, and `many_observables`.
- Cover hypergraph `no_error`, `distance_1`, `distance_2`, `distance_3`, `hyper_error`, `surface_code`, `repetition_code`, and `many_observables`.
- Cover shortest and likeliest WCNF cases for empty models, detector-only models, observable-only models, detector plus observable models, large probabilities, and half probability.
- Use exact output for stable WCNF and deterministic DEM text, and canonical target-set or minimum-weight structural comparators for tie-sensitive search output.
- Route folded DEM access through PFM-B3 and document caps for genuinely exponential search spaces.

Sparse tracker and matched errors:

- Use the generic loop summary in analyzer and unsigned-flow consumers instead of maintaining separate fixture-specific cycle logic.
- Add generated equivalence tests against small unrolled Clifford, measurement, detector, and observable loops.
- Keep active matched-error canonicalization and value validation covered, but leave stack-frame provenance, heralded provenance, repeat-contained provenance, and `explain_errors` deferred.

Benchmarks:

- Replace period-specific benchmark interpretation with `transient`, `short-period`, `long-period`, `nested`, and `generated-QEC` loop submeasurements.
- Keep graphlike, hypergraph, and SAT rows split by direct DEM and generated-QEC workloads.
- Record represented iterations, state size, emitted DEM items, search nodes or clauses, peak live bytes, and sampled resident delta.
- Promote only faithful stable rows into the 1.25x gate; keep structural or no-faithful-baseline rows report-only.

Acceptance criteria:

- No production analyzer branch matches a test fixture's exact instruction sequence or hard-coded recurrence period.
- Owned folded outputs match pinned Stim exactly where deterministic, and structural search comparators prove minimum-weight parity where ordering is non-contractual.
- Eligible huge repeats use memory bounded by loop state, body, and emitted compact output rather than repeat count.
- PFM6 closes for active analyzer, search, sparse-tracker, and matched-error value semantics while full provenance remains explicitly deferred.

### PFM-B6: Resolve Spec Gaps And Roll Up Status

Objective: turn completed blocker work into conservative, durable status without preserving stale `Partial` labels for already closed selected surfaces.

Tasks:

- Resolve all eight open entries in `docs/plans/milestone-spec-gaps.md` with links to implementation or evidence-close reports.
- Update checklist child rows to `Done for selected Rust/CLI scope` when their active work is complete.
- Keep broad product-level rollups partial only when a named deferred product surface still prevents a literal full-Stim claim; do not call that an active implementation blocker.
- Update the partial inventory, roadmap, test-porting plan, oracle manifest, benchmark manifest, profiler notes, waivers, and user-facing docs.
- Run milestone-audit for PFM-B1 through PFM-B5 and a final PFM8 rollup audit.
- Run full-code-review over shared gate semantics, traversal, flow, analyzer, search, CLI, benchmark, and documentation changes.

Verification:

- All milestone-specific tests and oracle rows from PFM-B1 through PFM-B5.
- `cargo fmt --all --check`
- `cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings`
- `cargo test --workspace --quiet`
- `just oracle::run --implemented-only`
- `just bench::smoke`
- `just maintenance::pre-commit`
- Fresh primary timing and memory evidence from current committed `HEAD` if any primary runner, threshold, or shared hot path changed.

Acceptance criteria:

- Every blocker has a resolved spec-gap entry and completion evidence.
- No active checklist row is partial merely because its wording once contained an unbounded adjective.
- Deferred Python, JS/WASM, diagram, ecosystem, simulator-product, GPU, exact-randomness, C++ header, full ErrorMatcher provenance, and deprecated `--detector_hypergraph` surfaces remain clearly separated from active Rust and CLI completion.

## Historical Detailed PFM Sections

The PFM0 through PFM8 sections below preserve the detailed evidence history that produced this blocker program.
Their phrases saying a surface is under-specified or awaits exact subcases are superseded by PFM-B0 through PFM-B6 and `docs/plans/blocker-closure-ledger.json`; use the ledger and blocker milestones for current execution and use the older sections only for implementation history, linked tests, and prior boundaries.

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

- Finish flow-semantic checks for `Circuit::decomposed` that depend on PFM5 measurement-rich flows. Selected MPP and pair-measurement decomposition flow-generator preservation is implemented; broader decomposition flow semantics should reopen only when a future exact PFM5 measurement-rich flow family is selected.
- Promote `Circuit::inverse_qec` beyond the unitary subset only for selected QEC inverse cases with exact owned subcases. The selected noiseless plain no-flow reset-measure-detector target-list shape, selected exact two-to-one `R`/`CX`/`M` detector-flow shape, selected exact `m_det` two-detector shape, selected exact noiseless `MPP` plus one-detector all-record identity-parity shape, selected direct `MPAD` record-tail shape, selected exact noisy `MZZ` detector-flow shape, selected exact observable Pauli include packet, selected noisy `M`/`MX`/`MY` measurement-only reversal, selected noisy `MR`/`MRX`/`MRY` measure-reset-only reversal, selected exact noisy measure-reset detector-flow packet, and selected measure-reset pass-through target-list shape are implemented for their selected detector counts, including pinned `r_m_det`, the exact one-qubit pinned `r_m_det_keep_m` options branch through `InverseQecOptions { keep_measurements: true }`, pinned `two_to_one`, pinned `m_det`, pinned `mpp`, pinned `mpad` direct inverse behavior, pinned `mzz`, pinned `obs_include_pauli`, pinned `noisy_m`, pinned `noisy_mr`, pinned `noisy_mr_det`, pinned `pass_through`, multi-target record remapping where selected, sparse detector subsets for the no-flow packet, duplicate detector-record parity where selected, selected duplicate MPAD record parity, selected duplicate MPAD observable-id record parity, empty-detector behavior where selected, and tag or coordinate preservation where applicable. Prior-measurement detector refs, MPAD observable flow terms outside selected record-only tails, MPAD Pauli-observable tails, duplicate MPAD observable-id merging with non-record targets, broader noisy measure-reset detector-flow, detector-flow rewrites with interleaved operations beyond the exact selected packets, broader observable-aware QEC inverse rewrites beyond the selected observable Pauli include packet, feedback, repeats, and multi-instruction QEC inverse behavior are under-specified in `docs/plans/milestone-spec-gaps.md` until exact subcases are selected.
- Promote `time_reversed_for_flows` beyond the current unitary Rust subset for the measurement-rich flow cases selected by PFM5 and the exact MPAD slice selected by PFM2. The selected single-instruction measurement-rich subset for one noiseless plain unique-target `M`, `MX`, `MY`, `MXX`, `MYY`, or `MZZ` instruction group, selected multi-record measurement-ordering examples, selected plain `R`, `RX`, and `RY` reset-to-measurement conversion over one or more unique qubit targets, selected single-target `M`, `MX`, and `MY` measurement-to-reset conversion including the selected `dont_turn_measurements_into_resets` single-measurement option, one noiseless `MR`, `MRX`, or `MRY` instruction over one or more unique qubit targets including inverted result targets, selected empty-flow plus Pauli-only, measurement-record, and observable MPAD record-tail reversal, including selected duplicate MPAD observable-id record parity tracked by `pf2-inverse-qec-mpad-rust`, the selected single-record `MZZ` plus plain-qubit unitary suffix packet matching pinned `flow_through_mzz_h_cx_s`, and the exact pinned `flow_flip` packet are implemented with pinned `M`, `R`, `MPAD`, `MZZ`, `dont_turn_measurements_into_resets`, and `flow_flip` examples plus source-owned basis, reset, measure-reset, MPAD rejection-boundary, and unitary-suffix coverage; duplicate reset-only and duplicate measure-reset behavior remain explicitly fail-closed under the boundary locked by [pfm2-time-reverse-duplicate-target-boundary-scope.md](pfm2-time-reverse-duplicate-target-boundary-scope.md) until a future compatibility decision selects bug-compatible Stim output, corrected semantic output, or permanent rejection, and MPAD observable flow terms outside selected record-only tails, non-selected detector or observable rewrites, feedback, noise, repeats, and broader multi-instruction QEC inverse behavior are under-specified in `docs/plans/milestone-spec-gaps.md` until exact subcases are selected.
- Extend feedback loop handling beyond the pinned `demolition_feedback`, pinned `interleaved_feedback_does_not_reorder_operations`, selected `XCZ`/`YCZ` measurement-record feedback, bounded pinned repeat-loop, and nested bounded-repeat detector-parity cases only when the exact repeat-contained feedback subcases, resource bounds, comparator, tests, oracle metadata, and benchmark evidence are specified. Broader repeat-contained feedback parity is under-specified in `docs/plans/milestone-spec-gaps.md` until that future exact-subcase plan exists.
- Preserve measurement record, detector, observable, coordinate, sweep, and repeat semantics across every transform.
- Keep materialized transforms capped or folded, and reject excessive expansion before building huge intermediate circuits.

Tests:

- Port owned transform cases from `vendor/stim/src/stim/circuit/circuit.test.cc`, `vendor/stim/src/stim/circuit/gate_decomposition.test.cc`, `vendor/stim/src/stim/util_top/transform_without_feedback.test.cc`, `vendor/stim/src/stim/util_top/circuit_flow_generators.test.cc`, and `vendor/stim/src/stim/util_top/has_flow.test.cc`.
- Add exact canonical-output tests for newly promoted transform cases.
- Add semantic tests that compare tableau action, detector error models, sampling distributions, or flow satisfaction before and after transforms.
- Add negative tests for unsupported feedback controls, unsupported repeat refolding, unsupported decomposition target shapes, invalid measurement-record rewrites, and excessive expansion.
- Add resource-boundary tests for nested large repeats and shift-only repeat folding.

Oracle rows:

- Supplement `pf2-circuit-decomposed`, `pf2-feedback-time-reverse`, and any transform row whose broad manifest-only coverage is replaced by executable evidence. `pf2-feedback-inline-pinned-upstream-rust` is the focused exact-output row for the pinned demolition-feedback and interleaved-ordering feedback cases, while `pf2-feedback-inline-scoped-rust` remains the broader selected feedback integration and rejection row.
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
- Expand analyzer sweep behavior beyond the original single no-op sweep-control subset only for selected exact subcases. The selected analyzer sweep-control matrix now covers `CX`, `CY`, `CZ`, `XCZ`, and `YCZ` no-op behavior, including `CZ` sweep/sweep, record/sweep, sweep/record, and record/record classical-only no-op groups, plus invalid controlled-Pauli target-position rejections; the boundary is locked in `docs/plans/pfm3-analyzer-sweep-boundary-scope.md`, and any broader analyzer sweep-shape parity is under-specified in `docs/plans/milestone-spec-gaps.md` until exact remaining gate-target shapes, comparator, CLI and Rust surfaces, oracle metadata, resource behavior, and benchmark policy are selected.
- Keep `m2d --sweep` and `--sweep_format` behavior synchronized with core converter behavior for every accepted input format. The current public CLI matrix covers `01`, `b8`, `r8`, `hits`, `dets`, and input-only `ptb64` sweep records under `pf3-m2d-sweep-format-matrix-cli`.
- Close sampler-backed target-order drift for Stim-parsed sweep targets: `CX q sweep[k]` and `CY q sweep[k]` must reject in reference sampling, detection conversion, non-frame detection sampling validation, and `stab m2d --sweep`, while sweep-first `CX` or `CY` and both-order `CZ` sweep/qubit groups must remain accepted.
- Classify legal gate execution support across sampler, converter, detection, and analyzer paths. The fixed-tableau gate contract is implemented for current sampler, detection-conversion, and analyzer surfaces, supported Hermitian `SPP` and `SPP_DAG` products are implemented for the promoted sampler, detection-conversion, detector-frame, and analyzer paths, selected deterministic `MPP` Pauli-product measurement execution and selected deterministic `MPAD` measurement-pad execution are implemented for the promoted sampler, detection-conversion, non-frame detection-sampling, frame detection-sampling, and analyzer paths, selected stochastic `MPP(p)` and `MPAD(p)` sampler and detection-sampling record-flip behavior is implemented for the promoted sampler, detection-converter reference mapping, non-frame detection-sampling, and frame-path detection-sampling surfaces, selected noisy `MPAD(p)` analyzer pad-flip effects are implemented for detector and observable DEM terms, and the selected boundary is locked in `docs/plans/pfm3-gate-semantic-boundary-scope.md`; broader legal non-tableau execution remains under-specified in `docs/plans/milestone-spec-gaps.md` until exact gate families, execution surfaces, comparator, resource behavior, oracle metadata, and benchmark policy are selected.
- Add precise errors for unsupported sweep target shapes, unsupported gate families, unsupported mixed feedback and sweep cases, and unsupported public output formats.
- Preserve streaming or documented caps for public inputs and outputs.

Tests:

- Port owned cases from `vendor/stim/src/stim/simulators/measurements_to_detection_events.test.cc`, `vendor/stim/src/stim/simulators/frame_simulator.test.cc`, `vendor/stim/src/stim/simulators/error_analyzer.test.cc`, `vendor/stim/src/stim/cmd/command_detect.test.cc`, and `vendor/stim/src/stim/cmd/command_m2d.test.cc`.
- Add sweep-record tests for `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64` wherever accepted. The accepted public `m2d --sweep_format` matrix is implemented by `m2d_accepts_sweep_records_in_all_text_and_byte_formats` and `m2d_accepts_sweep_records_in_ptb64_format`.
- Add semantic tests comparing sweep-conditioned circuits to explicit small-circuit expansions.
- Add target-order tests for sampler-backed sweep Pauli operations, including both rejected `CX q sweep[k]` and `CY q sweep[k]` cases and accepted sweep-first `CX` or `CY` plus both-order `CZ` cases.
- Add omitted-sweep default tests, width-mismatch tests, invalid-record-count tests, unsupported-format tests, unsupported-target-shape tests, and writer-error tests.
- Add gate execution tests that prove parser validation, sampler execution, detector conversion, detection sampling, and analyzer propagation do not drift.

Oracle rows:

- Supplement `pf3-sweep-m2d-detect`, `pf3-sweep-analyzer`, and `pf3-gate-semantic-execution` with executable rows for promoted subcases. `pf3-m2d-sweep-format-matrix-cli` is the executable row for the accepted public `m2d --sweep_format` input matrix, `pf3-gate-mpp-execution-rust` is the executable row for selected deterministic `MPP` execution evidence, `pf3-gate-mpp-stochastic-rust` is the executable row for selected stochastic `MPP(p)` sampler, detection-converter, non-frame detection-sampling, and frame-path detection-sampling evidence, `pf3-gate-mpad-execution-rust` is the executable row for selected deterministic `MPAD` execution evidence, `pf3-gate-mpad-stochastic-rust` is the executable row for selected stochastic `MPAD(p)` sampler, detection-converter, non-frame detection-sampling, and frame-path detection-sampling evidence, and `pf3-analyze-errors-mpad-noisy-cli` is the exact-output row for selected noisy `MPAD(p)` analyzer effects.
- `pf3-sampler-sweep-target-order-rust` is the executable structural row for sampler-backed `CX q sweep[k]` and `CY q sweep[k]` rejection plus accepted neighboring target orders.
- CLI rows must prove stdout, stderr class, exit status, accepted flags, rejected flags, path behavior, and resource behavior.

Benchmarks:

- Keep or refresh report-only rows `pf3-m2d-sweep-b8`, `pf3-m2d-sweep-ptb64-input`, `pf3-detect-sweep-sampling`, and `pf3-analyze-errors-sweep`.
- Keep the implemented `pf3-gate-semantic-wide` row report-only unless a faithful pinned-Stim comparator is added.
- Classify CLI rows as `cli-baseline` only when pinned Stim exposes the same command shape.
- Do not add a benchmark row for pure target-order rejection slices unless implementation changes a hot validation path.
- Do not add a benchmark row for selected deterministic `MPP` execution unless implementation changes a hot sampler, converter, detection-sampling, or analyzer path.
- Do not add a benchmark row for selected deterministic `MPAD` execution, selected stochastic `MPP(p)` or `MPAD(p)` sampler or detection-sampling evidence, or selected noisy `MPAD(p)` analyzer pad-flip effects unless implementation changes a hot sampler, converter, detection-sampling, or analyzer path.

Acceptance criteria:

- Every accepted sweep-conditioned path has core and public CLI evidence where the CLI exposes it.
- Every unsupported sweep shape fails before producing partial or misleading output.
- The sampler-backed `CX` or `CY` sweep target-order boundary matches pinned Stim execution behavior without narrowing flow-generator no-op semantics.
- Parser-accepted gates have documented execution behavior for every implemented execution surface, and broader legal non-tableau execution is not selected until an exact-subcase plan updates `docs/plans/pfm3-gate-semantic-boundary-scope.md` or replaces it with a sharper scope note.

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
- Finish selected folded coordinate behavior for large, nested, and ambiguous overlapping repeats, or keep documented caps with exact rejection tests. Selected large non-flat sparse coordinate holes now use actual declared-detector bounds to avoid impossible candidate iteration scans while preserving documented caps for genuinely ambiguous cases. The pinned Stim generated surface-code coordinate comparison is now counted only through the PFM6 bounded mixed-top-level analyzer fallback for prefix, repeat, and tail circuits under `fold_loops=true`; true folded generated-loop output is under-specified in `docs/plans/milestone-spec-gaps.md` until exact generated circuits, comparator behavior, resource boundaries, oracle metadata, and benchmark policy are selected.
- Finish folded or capped traversal behavior for DEM sampler, graphlike search, hypergraph search, SAT/WCNF generation, matcher-adjacent operations, and analyzer-adjacent operations where PFM4 owns the resource boundary. Graphlike and hypergraph search skip zero-probability repeated bodies, fold selected flat detector-touching and detectorless logical-only zero-shift repeated error bodies, skip selected flat no-target zero-shift repeated error bodies, fold selected flat zero-detector-shift `shift_detectors 0` repeated bodies, fold selected flat annotation-bearing repeated bodies with `detector` or standalone `logical_observable` declarations, skip selected flat and nested annotation-only zero-shift repeated bodies with `detector`, standalone `logical_observable`, and zero-detector-shift `shift_detectors` declarations including high sparse annotation ids that do not appear on error targets, fold selected nested zero-shift repeated bodies, fold selected mixed zero-probability plus active zero-shift repeated error bodies, sparsely index selected high-detector direct DEM search graphs, and keep caps for non-selected shifted active repeats plus shifted nested, non-flat, or broader non-annotation mixed-instruction active repeats; raw numeric `error` targets and separator-only `error` target lists are rejected at the DEM typed boundary and are locked by [pfm4-dem-search-invalid-target-boundary-scope.md](pfm4-dem-search-invalid-target-boundary-scope.md); ErrorMatcher filter DEM traversal folds selected flat and nested detector-touching plus detectorless logical-observable-only and annotation-bearing zero-shift repeated keys, skips empty and annotation-only neutral repeats, and keeps shifted active repeats, broader active mixed-instruction filter bodies, and repeat-contained circuit noise capped or rejected; weighted SAT/WCNF omits zero-probability error variables, skips repeated zero-probability and selected annotation-only bodies before flattening, folds selected flat and nested zero-shift repeat bodies by concrete MAP parity cost, and keeps a dense SAT target cap for shifted active errors; unweighted SAT folds selected flat and nested zero-shift repeat bodies structurally, including zero-probability and no-target mechanisms, and skips selected annotation-only zero-shift bodies including high sparse annotation ids that do not appear on error targets while high-index dense-target error mechanisms, shifted, non-flat, or other non-selected structural repeats remain capped, with dedicated high-observable dense-cap evidence for selected zero-probability structural error repeats; DEM sampler direct detection-event output skips zero-probability repeated bodies, folds deterministic zero-shift repeats by parity, folds selected single-stochastic zero-shift repeats by odd-parity probability, folds selected flat stochastic zero-shift repeats by per-error odd-parity probability, and folds selected nested zero-shift stochastic repeats by recursive effective-error parity.
- Preserve tags, separators, detector shifts, coordinate shifts, logical observables, repeat structure, and probability rounding contracts across public transforms.
- Keep all-detector materialization APIs capped when they must materialize large maps, and point callers to selected lookup APIs.

Tests:

- Port owned DEM cases from `vendor/stim/src/stim/dem/detector_error_model.test.cc`, `vendor/stim/src/stim/dem/dem_instruction.test.cc`, and Python DEM tests as semantic-mining sources.
- Add exact canonical-output tests for `flattened`, `rounded`, `without_tags`, tags, separators, detector shifts, coordinate shifts, logical observables, and repeats.
- Add structural tests for selected-coordinate lookup, all-detector coordinate maps, final coordinate shifts, final detector shifts, detector counts, observable counts, and error counts through large repeats.
- Add negative tests for invalid probabilities, invalid separators, invalid coordinate values, invalid repeat counts, detector-shift overflow, high ids, unsupported transform shapes, and non-finite folded coordinate results.
- Add resource-boundary tests for huge repeats, nested repeats, ambiguous overlapping repeats, and every consumer that still expands within a cap.

Oracle rows:

- PFM-B3 supplements the historical `pf4-dem-introspection-transforms` and `pf4-dem-coordinate-api` evidence and promotes `pf4-dem-folded-traversal` to an implemented umbrella with seven focused executable child rows.
- Exact rows should cover stable `.dem` text outputs.
- Structural rows should cover folded traversal, caps, and resource behavior.

Benchmarks:

- Keep or refresh report-only rows `pf4-dem-flatten-repeat`, `pf4-dem-rounded`, `pf4-dem-coordinate-map`, `pf4-dem-folded-traversal`, `pf4-dem-folded-graphlike-traversal`, `pf4-dem-hypergraph-logical-repeat`, `pf4-dem-hypergraph-no-target-repeat`, `pf4-dem-search-zero-shift-repeat`, `pf4-dem-search-annotation-repeat`, `pf4-dem-search-mixed-zero-probability-repeat`, `pf4-dem-search-nested-repeat`, `pf4-dem-sat-flat-repeat-fold`, `pf4-error-matcher-filter-flat-repeat`, `pf4-error-matcher-filter-nested-repeat`, `pf4-error-matcher-filter-logical-repeat`, `pf4-error-matcher-filter-annotation-repeat`, and `pf4-dem-sampler-folded-repeat`.
- Promote only faithful direct-match rows with repeated stable evidence.
- Do not add a separate benchmark row for selected annotation-only search/SAT/WCNF repeat skipping, including high sparse annotation ids, unless it becomes a measurable hot path; the current slice is admission/resource evidence for avoiding repeat-count-scaled work and annotation-only dense-target cap pressure.
- Record measurement work units for detector count, detector coordinate lookup, flattened instruction count, skipped zero-probability error occurrences, skipped no-target error occurrences, folded zero-detector-shift target-error occurrences, folded annotated target-error occurrences, folded active target-error occurrences for mixed zero-probability search repeats, folded nested target-error occurrences, folded filter-key occurrences, folded nested filter-key occurrences, folded logical filter-key occurrences, folded annotated filter-key occurrences, folded deterministic error occurrences, folded stochastic error occurrences, folded flat stochastic error occurrences, folded nested stochastic error occurrences, folded SAT error occurrences, folded nested SAT error occurrences, and sampled or searched model size.

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

- Extend `circuit_detecting_regions` for selected Clifford gates, target shapes, tick windows, detector or logical-observable filtering, multi-detector regions, anticommutation behavior, gauge behavior, and repeat traversal. The bounded repeat traversal, detector and logical-observable `DemTarget` filters, dense-capped default-like all-target/all-tick helpers, pinned `MX`, `MZZ`, and tagged ordinary-noise detecting-region examples, generated repetition-code all-target/all-tick selection with selected exact D0, D6, and L0 regions, selected generated memory-Z and memory-X surface-code all-target/all-tick helper counts plus exact D0, D4, and L0 regions, the full single-qubit Clifford gate set with plain qubit targets, fixed two-qubit tableau-backed Clifford gates with plain qubit-pair targets, selected measurement-record feedback placements for `CX`, `CY`, `CZ`, `XCZ`, and `YCZ`, selected gate-order-valid sweep-controlled Pauli sign-only no-op groups for `CX`, `CY`, `CZ`, `XCZ`, and `YCZ` plus selected `CZ` classical-only bit-bit groups, explicit fail-closed validation for non-`CZ` sweep/sweep, record/sweep, and record/record groups, ordinary non-record-producing noise no-op traversal, inverted targets for promoted measurement and reset-measurement families, `MPAD` measurement pads, `MPP` Pauli-product measurement targets, `SPP` and `SPP_DAG` unitary Pauli-product target shapes, `HERALDED_ERASE` and `HERALDED_PAULI_CHANNEL_1` record-producing noise with plain qubit targets, ignored anticommutation mode, selected measurement-gauge ignored-mode behavior, and product-measurement gauge-cancellation behavior are implemented; broader target shapes outside the promoted positive set and source-owned fail-closed set, broader generated-code regions beyond the promoted repetition-code and selected memory-Z and memory-X surface-code cases, and broader gauge behavior are under-specified in `docs/plans/milestone-spec-gaps.md` until exact subcases are selected.
- Keep `missing_detectors` evidence current for the promoted basic reset/measure suggestions, Gaussian row reduction for multi-record detector rows, repeated MPP and pair-measurement stabilizer-product cases, record-only observable rows, ignored Pauli observable rows, `MPAD` measurement-pad records, tableau-backed single-qubit and fixed two-qubit Clifford propagation with plain qubit target groups, `SPP` and `SPP_DAG` unitary Pauli-product analysis through decomposition, bounded repeat traversal with explicit expansion caps, selected folded final-repeat traversal for covered deterministic measurement loops with flat or bounded nested local bodies, selected observable-neutral final repeats where top-level record-only observable rows are redundant under independent detector evidence, and pinned honeycomb and toric global-stabilizer suffix cases. The generated-code suffix boundary is locked in `docs/plans/pfm5-missing-detectors-generated-boundary-scope.md`, and broader MPP, pair-measurement, observable, gauge, Clifford-propagation, repeat-traversal, folded large-repeat, row-reduction, unknown-input, and generated-code suffix families outside the promoted evidence are under-specified in `docs/plans/milestone-spec-gaps.md` until exact subcases, comparators, resource behavior, oracle metadata, and benchmark policy are selected.
- Keep the basic `Flow` object surface closed by `coverage-stabilizers-flow`: parsing, canonicalized measurement indices, included observables, display, ordering, and multiplication are already covered and should be reopened only if the Rust API changes. Implement measurement-rich `has_flow`, `has_all_flows`, `flow_generators`, `solve_for_flow_measurements`, diagnostics where selected, signed sampled checking if selected, folded traversal, and transform integration for the selected Rust scope. The `M`/`MX`/`MY`, `R`/`RX`/`RY`, `MR`/`MRX`/`MRY` including inverted result targets, `MXX`/`MYY`/`MZZ`, Python multi-target `M`/`MX`/`MYY`/`MPP`, nonconstant and constant single-instruction `MPP`, pinned variable-target `SPP` and `SPP_DAG` unitary generator examples, selected unitary-mixed composed measurement-rich instruction sequences, selected all-operation annotation and ordinary-noise no-op traversal, selected composed `SPP` and `SPP_DAG` unitary decomposition, the pinned generated all-operations fixture, bounded repeat-contained measurement-rich instruction sequences, `MPAD`, scoped measurement-record feedback, selected gate-order-valid sweep-controlled Pauli sign-only no-op groups, selected single- and multi-target heralded-noise MPP `circuit_flow_generators`, unsigned `has_flow` and `has_all_flows` Rust helpers for the promoted checker subset, an additive unsigned diagnostic checker for output mismatches, input mismatches, out-of-range measurement records, and unsupported-circuit reasons, scoped signed sampled flow checking for unitary, measurement-record, record-backed observable, Pauli-backed observable, inverted Pauli-backed observable, and inverted record-backed observable cases, and pinned Stim empty, `MX`, measured-idle, multi-target measurement and `MPP`, fewer-measurements heuristic, and repetition-code `solve_for_flow_measurements` examples are implemented; broader all-operation composed measurement-rich generators beyond the promoted pinned generated all-operations fixture, no-op, tableau, measurement-record feedback, selected gate-order-valid sweep-controlled Pauli sign-only no-op groups, heralded-record, and `SPP` or `SPP_DAG` decomposition subcases, broader heralded-noise generator synthesis beyond the selected MPP cases, folded repeat traversal beyond the current flow-row and materialized flattened-operation caps, and broader solver or generator diagnostics are under-specified in `docs/plans/milestone-spec-gaps.md` until exact subcases are selected.
- PFM-B4 supersedes the historical solver limitation above: supported generator tables now use general GF(2) elimination without measurement-subset enumeration, and the remaining under-specification applies to generator families, diagnostic wording, and repeat caps outside the finite ledger, not to matrix size. Reverse-flow transform integration is owned by PFM-B1.
- The historical packet-specific `time_reversed_for_flows` bridge described by earlier PFM5 work is superseded by PFM-B1. The current selected Rust transform scope uses one tracker-driven gate-family engine for all nineteen finite ledger cases, including multi-instruction detector and observable remapping, measurement/reset/measure-reset and Pauli-product families, MPAD record and observable parity, generated surface-code reversal, exact pair-aware target reversal, sparse high-qubit tracking, folded unitary repeats, and bounded measurement-rich repeats. Feedback, heralded-record reversal, and duplicate collapse targets remain fail-closed, and behavior outside the finite PFM-B1 ledger requires an explicit future plan instead of being described as generically broader work.
- Add precise errors for unpromoted utility families.

Tests:

- Port owned cases from `vendor/stim/src/stim/util_top/circuit_to_detecting_regions.test.cc`, `vendor/stim/src/stim/gen/gen_surface_code.test.cc`, `vendor/stim/src/stim/util_top/missing_detectors.test.cc`, `vendor/stim/src/stim/stabilizers/flow.test.cc`, `vendor/stim/src/stim/util_top/circuit_flow_generators.test.cc`, `vendor/stim/src/stim/util_top/circuit_flow_generators_test.py`, and `vendor/stim/src/stim/util_top/has_flow.test.cc`.
- Add positive and negative tests for each promoted detecting-region gate, target shape, filter mode, multi-detector case, gauge case, and repeat case.
- Keep exact positive and negative tests for the promoted `missing_detectors` row-reduction, Pauli-product, observable, `MPAD`, Clifford-propagation, `SPP` and `SPP_DAG`, bounded repeat, selected folded final-repeat, selected observable-neutral final-repeat, honeycomb, and toric suffix evidence. Do not add broader MPP, pair-measurement, observable, gauge, Clifford-propagation, repeat-traversal, folded large-repeat, row-reduction, unknown-input, or generated-code missing-detector tests until an exact-subcase plan selects circuits, suffix comparators or source-owned invariants, resource behavior, oracle metadata, and benchmark policy.
- Keep the existing `coverage-stabilizers-flow` tests for `Flow` object parsing, measurement records, observables, multiplication, validation, and sign behavior current when the object API changes; add or refresh flow tests for promoted `has_flow`, generator solving, solve-for-measurements, failure paths, signed sampled behavior if selected, transform integration, and diagnostic quality. The current unsigned diagnostic checker is covered by `pf5-has-flow-diagnostics-rust`; future diagnostic rows should be added only when solver, generator, signed sampled, or transform diagnostics are selected.
- Add transform-integration tests proving flow-aware transforms preserve or intentionally rewrite flow data.

Oracle rows:

- Supplement `pf5-detecting-regions-extended`, `pf5-missing-detectors-extended`, and `pf5-measurement-rich-flows`.
- Keep `pf5-detecting-regions-generated-unrotated-surface-rust` synchronized with the selected unrotated surface-code D0, D4, and L0 exact-output slice.
- Keep `pf5-detecting-regions-generated-surface-memory-x-rust` synchronized with the selected rotated and unrotated memory-X surface-code D0, D4, and L0 exact-output slice.
- Use structural comparators for set-like results or ordering-insensitive outputs.
- Keep Python binding ergonomics out of the oracle claim.

Benchmarks:

- Keep or refresh `pf5-detecting-regions-repeat`, `pf5-detecting-regions-targets`, `pf5-detecting-regions-clifford`, `pf5-detecting-regions-generated-repetition`, `pf5-detecting-regions-generated-surface`, `pf5-missing-detectors-mpp`, `pf5-missing-detectors-mpad`, `pf5-missing-detectors-generated-code`, `pf5-has-all-flows-batch`, `pf5-flow-generators-measurement-rich`, `pf5-flow-generators-measurement-python`, `pf5-flow-solve-measurement-rich`, and `pf5-flow-solve-measurement-python`.
- Do not add separate unrotated memory-Z or memory-X surface-code detecting-region benchmark rows unless a future workload exercises a distinct performance path; the current additional surface-code slices are exact-output test and oracle evidence only.
- Keep rows report-only unless faithful Stim comparison and repeated stable ratios exist.

Acceptance criteria:

- Every promoted utility subfamily has positive, negative, and resource-boundary tests, except the locked generated-code missing-detector boundary whose current acceptance is the two exact positive pinned suffix cases plus an explicit under-specification log for broader generated-code negative and resource-boundary behavior.
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

- Extend `circuit_to_detector_error_model` for selected generated circuits, loop folding, gauge detectors, approximate disjoint errors, decomposition options, remnant-edge blocking, and ignored decomposition failures. The selected prefix, repeat, and tail detector-chain shape under `fold_loops=true` now has true compact folded output after candidate validation against a measurement-record-lookback-sized non-folded expansion, including the selected large repeat-count resource case. The selected pinned loop-carried observable shape now has true compact folded output for the huge odd-repeat case where tail `OBSERVABLE_INCLUDE` annotates the repeated body errors after the same validation style. The selected pinned period-8 logical-observable oscillation shape now has exact Stim-style compact folded output after validation against a non-folded nine-iteration expansion. The selected pinned period-127 logical-observable oscillation shape now has exact Stim-style compact folded output with its deterministic tail detector after validation against a non-folded 338-iteration expansion, while the 338-iteration single-middle-repeat boundary stays on the bounded non-folded path to preserve Stim output shape. Delayed measurement-record dependencies that the selected compact fold cannot prove fall back to the bounded non-folded analyzer. The selected generated surface-code prefix, repeat, and tail coordinate shape still uses a bounded mixed-top-level fallback that reuses the capped non-folded analyzer, which is enough to prove the pinned Stim coordinate comparison but does not claim true folded output for broader generated-loop families. The selected upstream guard for folded observable dependencies that cross iterations without including every loop-carried measurement is implemented as a nondeterministic-observable rejection, not folded-output support.
- Extend graphlike, hypergraph, shortest-error, SAT, and WCNF behavior for selected generated-circuit and direct DEM cases.
  The selected direct DEM graphlike and hypergraph exact-output subset is implemented by `pf6-search-direct-dem-graphlike-rust` and `pf6-search-direct-dem-hypergraph-rust`, the selected direct DEM sparse high-detector resource subset is implemented by `pf6-search-sparse-high-detectors-graphlike-rust` and `pf6-search-sparse-high-detectors-hypergraph-rust`, the selected high-observable analyzer-to-search subset is implemented by `pf6-search-many-observables-graphlike-rust` and `pf6-search-many-observables-hypergraph-rust`, the selected generated-QEC graphlike and hypergraph search subset is implemented by `pf6-search-generated-qec-rust`, and the selected generated-QEC SAT/WCNF structural subset is implemented by `pf6-search-generated-sat-wcnf-rust`.
- Add or extend ordering-insensitive structural comparators for search outputs where exact target order is not stable. The selected generated-QEC graphlike and hypergraph search rows now include canonical target-set uniqueness, deterministic error rows, zero detector parity, and exact `L0` observable parity; broader generated families and tie-sensitive target-set comparators are under-specified in `docs/plans/milestone-spec-gaps.md` until exact subcases are selected.
- Improve sparse reverse detector-frame tracking for optimized loop folding and analyzer or search correctness. The supported-Clifford unitary-repeat folding subset is implemented for the full single-qubit Clifford gate set and fixed two-qubit tableau-backed Clifford gates with plain qubit-pair targets, with deterministic generated repeat tests covering nested repeats, multi-target single-qubit instructions, multi-pair two-qubit instructions, and no-fold traversal comparisons. The shifted-copy measurement/detector repeat subset is implemented by `pf6-sparse-rev-shifted-repeat-rust`, covering record and detector offset comparison, shifted target application, small unrolled equivalence, public unsigned-flow consumption, and a trillion-iteration period skip. The unsigned sparse-tracker path also supports `SPP` and `SPP_DAG` product propagation for public unsigned-flow checking. Analyzer/search-specific consumption and broader variable-target unitary semantics outside this unsigned tracker path are under-specified in `docs/plans/milestone-spec-gaps.md` until exact subcases are selected.
- Harden matched-error value objects only where active analyzer/search outputs require them. The selected `ExplainedError` and `CircuitErrorLocation` canonicalization slice is implemented by `pf6-matched-error-canonicalize-rust`; broader hardening should reopen only when a future analyzer or search output selects exact value-object fields and comparators.
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
- Keep `pf6-analyzer-loop-carried-observable-rust` and `pf6-analyze-errors-loop-carried-observable-cli` as evidence only for the selected pinned huge odd-repeat loop-carried observable output they name.
- Keep `pf6-analyzer-period8-observable-rust` and `pf6-analyze-errors-period8-observable-cli` as evidence only for the selected pinned period-8 logical-observable oscillation output they name.
- Keep `pf6-analyzer-period127-observable-rust` and `pf6-analyze-errors-period127-observable-cli` as evidence only for the selected pinned period-127 logical-observable oscillation output they name.
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
- Keep `pf6-error-decomp-loop-folded` synchronized with the promoted repeated composite-error loop-folded decomposition subset, keep `pf6-analyzer-loop-observable-folded` synchronized with the promoted selected loop-carried observable folding subset, keep `pf6-analyzer-period8-observable-folded` synchronized with the promoted selected period-8 logical-observable folding subset, keep `pf6-analyzer-period127-observable-folded` synchronized with the promoted selected period-127 logical-observable folding subset, and extend or split them if broader decomposition or observable-period families become active; keep the implemented `pf6-sparse-rev-frame-loop` row report-only unless a faithful pinned-Stim comparator is added.
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

Current evidence:

- `docs/plans/pfm8-rollup-evidence-report.md` records the current PFM8 rollup evidence snapshot. It is intentionally not a final PFM8 completion report because PFM-B1 still needs final review and clean evidence, PFM-B2 and PFM-B5 retain implementation work, and PFM-B6 must audit and roll up the completed children; PFM-B3 and PFM-B4 are already complete.

## Final Verification

Before claiming the whole plan complete, run:

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings
cargo test --workspace --quiet
just oracle::blockers --check-selectors
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
