# Stim Core Parity And Lean Evidence Plan

Status: Active. P0 completed in `07ebf4c8`; P1 through P3 are complete; P4 through P9 have not started.

## Summary

Complete semantic feature parity with Stim v1.16.0 for Stab's core Rust libraries and command-line workflows, excluding deprecated Stim surfaces, documented Stim bugs, and the explicitly deferred product areas below. Use this work to finish the architectural cleanup already started by the component-crate split, then replace the current qualification bureaucracy with a small behavior-oriented correctness suite and one end-to-end performance system.

This is a deliberate pre-1.0 contract reset. Existing Stab APIs, test ledgers, benchmark schemas, and operational commands may be removed instead of receiving compatibility shims when a cleaner direct design replaces them. Stim-visible semantics remain the target; compatibility with obsolete Stab-only interfaces is not a goal.

## Rationale

The current crate split is a sound product architecture. `stab-model`, `stab-records`, `stab-bits`, `stab-algebra`, `stab-analysis`, `stab-engine`, `stab-decoder`, `stab-kernels-simd`, `stab-cli`, and the `stab-core` facade already separate the major concerns well enough. Adding more product crates would mostly move complexity instead of removing it.

The remaining entropy is concentrated elsewhere:

- `stab-core` still carries compatibility routes, duplicate exports, facade-owned behavior, and error translation that obscure the real component owners.
- Some public abstractions describe imagined backends or old materializing APIs instead of the one supported execution model.
- Correctness ownership is spread across API-item inventories, qualification selectors, compatibility matrices, fixtures, reports, and prose status documents.
- Performance evidence is split across legacy milestone rows and a newer qualification system, while many planned microbenchmarks do not represent a user workflow.
- Historical plans and progress reports are still easy to mistake for active requirements.

The cleanup must preserve the parts that prove real risk: strict result-format grammars, typed DETS layouts, path-alias data-loss prevention, bounded subprocess supervision, pinned-Stim differential testing, deterministic resource limits, output validation, paired timing, and memory measurement.

## Scope

### Included

- Both frozen Stim v1.16.0 text models: circuits and detector error models.
- Every nondeprecated gate, alias, argument rule, legal target shape, tag behavior, and repeat-block behavior in the frozen gate catalog.
- Circuit parsing, printing, inspection, generation, transformation, flow analysis, sampling, detection conversion, and detector error model conversion.
- Detector error model parsing, printing, transformation, sampling, search, and satisfiability behavior that belongs to the selected core API.
- The `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64` result formats, including materialized, streaming, packed, sparse, and typed-layout workflows where those representations are useful.
- The seven computational CLI commands `gen`, `convert`, `sample`, `detect`, `m2d`, `analyze_errors`, and `sample_dem`, plus the Stim `help` discovery surface. Stab's concise implemented-only help text is an explicit divergence from Stim's bundled documentation.
- Idiomatic Rust public APIs for the same stable capabilities. Stim's stable Python API defines relevant behavior, but Python object shape and source compatibility are not goals; C++ implementation and tests provide semantic examples, not a stable API contract.
- Stab-native agent inspection commands, JSON Lines diagnostics, decoder sessions, external circuit passes, and `.stim -> .stim` canonical conversion remain tested extensions, but they do not count toward Stim parity completion.
- Safe typed limits for hostile input, allocation, recursion, output size, and process control. A deliberate limit may differ from Stim when the ledger documents the divergence and tests the boundary.

### Deferred

- Python bindings.
- JavaScript and WebAssembly bindings.
- Ecosystem integrations such as Cirq, PyMatching, Sinter, and Crumble.
- `diagram`.
- `explain_errors` and full `ErrorMatcher` provenance.
- `repl` and interactive simulator products.
- Public `TableauSimulator` and `FlipSimulator` products; their internal algebra and frame semantics remain required by batch workflows.
- QASM, Quirk, and other export surfaces.
- GPU execution.
- Exact reproduction of Stim's random bit streams.

Deprecated Stim functions and flags are omitted instead of deferred. Confirmed Stim bugs are not copied silently; each intentional correction must be recorded as an explicit divergence with a pinned reproduction and a Stab regression test.

### Deprecated Exclusions

| Surface | Reason for exclusion | Pinned source |
|---|---|---|
| Legacy top-level dispatch flags `--sample`, `--detect`, `--gen`, `--m2d`, and `--analyze_errors` | Superseded by named subcommands; P6 removes Stab's compatibility normalization instead of preserving a second parser path. | `src/stim/main_namespaced.test.cc` and command-specific tests |
| `--detector_hypergraph` | Deprecated Stim spelling for `analyze_errors`; the named command is the supported surface. | `src/stim/cmd/command_analyze_errors.test.cc` |
| `sample --frame0` | Deprecated alias for `--skip_reference_sample`. | `src/stim/cmd/command_sample.cc` |
| `detect --prepend_observables` | Deprecated observable ordering that Stim itself warns against. | `src/stim/cmd/command_detect.cc` |

Stim's MBQC decomposition helper is also excluded because it feeds rich help rendering instead of a stable computational API. Stab's concise structural help is already an explicit divergence. Python-only deprecated aliases are outside the selected Rust and CLI product boundary.

## Target Product Architecture

### Component Ownership

- `stab-model` owns circuit and detector error model syntax, typed targets, metadata, structural transforms, and canonical rendering.
- `stab-records` owns result formats, layouts, readers, writers, bounded streaming, and format conversion.
- `stab-bits` owns generic packed and sparse bit storage.
- `stab-algebra` owns Pauli, Clifford, tableau, and flow algebra.
- `stab-analysis` owns circuit-to-DEM lowering, flow and detector analysis, inverse and reverse analyses, search, and satisfiability.
- `stab-engine` owns compilation and execution plans, sampling sessions, detection conversion sessions, and detector error model sampling sessions.
- `stab-decoder` owns Stab-native decoder contracts and reusable decoder sessions; this is an extension boundary, not a Stim parity requirement.
- `stab-kernels-simd` owns measured SIMD leaf kernels only.
- `stab-cli` owns argument parsing, file-role preflight, process-level workflows, help, and exit behavior.
- `stab-core` is a thin convenience facade. It may re-export stable common types and direct component namespaces, but it must not own algorithms, duplicate data models, or translate every domain error into a facade error.

The CLI should depend directly on the component crates that own each operation. The facade exists for users who prefer one dependency, not as an internal service locator.

### Public API Reset

- Remove `advanced`, `experimental`, `advanced::compat`, duplicate root exports, forwarding-only modules, and facade-only implementations.
- Remove the catch-all `CircuitError`; return the owning parser, model, analysis, engine, record, or decoder error.
- Remove `SampleFormat` and use one `RecordFormat` type.
- Remove legacy `CompiledSampler`, `CompiledDetectionConverter`, and `CompiledDemSampler` adapters that materialize whole outputs or expose callback-specific compatibility routes.
- Standardize reusable work on `compiler -> immutable plan -> mutable session -> batch/sink`.
- Remove public backend-selection knobs until two executable backends actually exist.
- Keep SIMD selection private and build-time or runtime-internal; do not expose an unavailable `PortableSimd` sampling backend as a product choice.
- Keep `CircuitPass` and `DecoderSession` because they represent real extension and state-reuse boundaries.
- Keep agent-facing `capabilities`, `inspect`, and `plan` commands because they expose the real component graph and supported operations without creating a second execution API.

## Single Parity Source Of Truth

P0 introduces `oracle/stim-v1.16-parity.toml` as the sole current feature-parity ledger manifest. It names bounded, normalized family fragments under `oracle/stim-v1.16-parity/` so the logical ledger remains one validated source without creating an oversized contract file. Each entry records:

- a stable behavior-family ID;
- the user-observable contract;
- one implementation status: `done`, `missing`, `deferred`, or `divergence`;
- the owning product crate;
- a separate evidence status: `verified` with one canonical semantic test selector, or `needs-owner` with the milestone that replaces broad historical evidence;
- the pinned Stim source or test references;
- the completion milestone when behavior is `missing`;
- a rationale when status is `deferred` or `divergence`.

There is no `partial` status. A broad row that contains both implemented and missing behavior must be split until every row has one honest implementation status. Missing canonical evidence never changes an implemented row back to `missing`; it changes only the evidence state. Entries describe behavior families, not every Rust export, every upstream test file, or every fixture.

The ledger generates `docs/stim-parity.md`. That generated document replaces `docs/stim-feature-list.md`, `docs/stab-feature-checklist.md`, the blocker ledger, and hand-maintained feature counts as the current status view. Historical files remain until P9, when their durable rationale is extracted and the superseded files are deleted.

## Milestone P0: Freeze Scope And Replace Status Ledgers (Complete)

### Tasks

1. Add this plan and make `GOAL.md` the short active execution contract.
2. Build the parity ledger from the pinned Stim source, current feature documents, public Rust APIs, CLI command definitions, and explicit unsupported-path errors.
3. Split every currently partial row into atomic behavior families, classify its implementation state, and record evidence readiness separately.
4. Cover both model dialects, all 81 canonical instructions, all 12 aliases and legal target families, six result formats, seven computational commands, help, generators, transforms, sampling, conversion, analysis, search, algebra, and resource boundaries.
5. Record an explicit command, role, record-type, and format applicability matrix so a general codec claim cannot silently stand in for an unsupported CLI route.
6. Mark all older implementation and qualification plans as superseded by this plan without deleting history yet.
7. Add Rust operations and thin `just` recipes for `oracle::parity-check`, `oracle::parity-run`, and `oracle::parity-render`.
8. Generate `docs/stim-parity.md` deterministically and make CI reject drift.
9. Derive implemented command-option validation from `stab_cli::command_descriptor()` so removing an option from the live Clap parser invalidates its parity claim even when the frozen Stim map and ledger still agree.

### Tests

- Reject duplicate IDs, unknown statuses, missing rationales, missing owners, stale test selectors, and mismatched Stim identities.
- Prove that every frozen gate and alias, each of the six result formats, both text dialects, and every in-scope CLI command appears in exactly one complete behavior-family partition.
- Prove that generated Markdown is byte-stable and changes only when the ledger or renderer changes.
- Run each `verified` selector independently; a broad crate or file pass is supporting evidence only.

### Benchmarks

No timing work is allowed in P0. The future E2E workload families are named and mapped to parity rows, but performance claims remain historical until P7.

### Done Criteria

- Every in-scope behavior is honestly classified and no `partial` row remains.
- Each implemented row is visibly either `verified` or `needs-owner`, and each verified row has one meaningful semantic owner.
- `GOAL.md`, this plan, the parity ledger, and generated parity view are the only active scope/status sources.
- A milestone audit finds no hidden nondeferred behavior outside the ledger.

P0 completed in `07ebf4c8` with 132 atomic families, 50 independently executable canonical owners, live Clap option validation, bounded modular family fragments, deterministic rendering, and CI drift checks. The milestone audit and full-code-review findings were fixed before the source commit.

## Milestone P1: Build A Lean Correctness Suite

### Tasks

1. Organize tests by behavior family and product owner instead of qualification bureaucracy.
2. Preserve the independent 62-case result-format corpus, path-alias matrix, bounded process-supervisor tests, decoder conformance tests, and external circuit-pass proof.
3. Replace manually mirrored gate expectation tables with canonical model metadata plus pinned-Stim differential checks.
4. Move shared corpus schema and decoding support into one internal test-support owner; leave semantic assertions in the crate and CLI that own the behavior.
5. Delete per-export qualification cases, structural-only fixtures, duplicate compatibility rows, and tests whose only claim is a derive, type name, re-export, constant, marker inequality, static label, or private pointer identity.
6. Remove semantic ownership from the old qualification machinery once the parity-ledger owners pass. Keep a finite source-owned catalog containing only the 48 exact correctness prerequisites consumed by active runtime groups, plus its mechanically generated IDs and digest. It must contain no upstream, public-API, checklist, or oracle-fixture ownership and must be deleted with the inherited benchmark system in P7.
7. Leave new model-wide parse byte, instruction-count, and target-count limits to P3, where their public policy, exact boundaries, Stim divergence, and hostile-input behavior can be specified together. P1 may consolidate tests for existing limits but must not invent new product behavior to make a resource row appear complete.

### Tests

- Every parity row has exactly one canonical owner test and, when the boundary is public CLI behavior, one real-process CLI assertion.
- A canonical owner may be one concise table-driven or generated matrix when all cases exercise one cohesive public contract. Split tests at distinct semantic, failure, cancellation, or resource boundaries, not once per upstream test case.
- Gate-family tests cover all legal target and argument shapes from canonical metadata, plus representative invalid shapes.
- Property tests use fixed seeds and persisted minimal regressions for parser/printer round trips, tableau identities, folded versus unrolled repeats, chunking invariance, and codec equivalence.
- Resource tests assert bounded allocations, bounded retained capacity, prompt cancellation, and no record-count growth instead of allocator pointer identity.
- Differential tests compare accepted output or rejection class against pinned Stim; round trips never own compatibility by themselves.

### Benchmarks

Only untimed correctness smoke may run. Tests must not contain timing assertions.

### Done Criteria

- Deleting any surviving test would remove a named semantic, safety, statistical, or resource-boundary claim.
- No test-support representation mirrors a production model unnecessarily.
- Each defect has one minimal owner regression and only the additional boundary test needed to prove propagation.
- The lean suite covers every `done` parity row without depending on historical qualification selectors.

### Implementation Checkpoint

- Current family, status, divergence, and canonical-owner totals are generated in `docs/stim-parity.md`; this plan does not duplicate those volatile counts.
- The old per-export and structural-only test owners have been removed or consolidated. Strict result-format grammar, path-alias, bounded process, decoder-session, external-pass, statistical, and resource-boundary evidence remains intact.
- The finite correctness bridge is generated directly from its 48 standalone benchmark prerequisites, rejects retired ownership state, and must exactly equal the runtime prerequisite union in both directions.
- Fixed-seed algebra and codec properties, a bounded deterministic parser corpus, exact logical-search admission and first-excess checks, and retained-state-term ordering close the final audit findings.
- Formatting, warnings-denied workspace Clippy, all workspace tests, live and checked oracle contracts, parity owners, correctness PR and full tiers, benchmark contracts, status generation, benchmark smoke, and diff checks pass. The post-fix milestone audit and full code review report no remaining P1 blocker.
- The historical correctness inventory remains only as a generated compatibility bridge for inherited benchmark prerequisites. It is not a feature-status or semantic-ownership source.
- P1 is complete only after generated-contract checks, workspace formatting, Clippy, all tests, benchmark smoke, pre-commit, milestone audit, and full code review pass on the final diff.

## Milestone P2: Finish The Public Architecture Reset

### Tasks

1. Make `stab-cli` use the owning component crates directly.
2. Move facade-owned algorithms to their proper component owners and leave `stab-core` as a thin re-export layer.
3. Replace catch-all errors with owning domain errors and typed context.
4. Consolidate result-format naming on `RecordFormat`. Completed: the duplicate `SampleFormat` type and its conversions are removed, generic readers accept all six formats, and one-record writers reject grouped-only PTB64 construction.
5. Replace legacy compiled/materialized/callback adapters with one compiler-plan-session-optional transaction-batch/sink model. Transactions exist only where several incremental calls must share one sink finalization lifecycle.
6. Remove backend placeholders, unavailable backend choices, compatibility descriptors, duplicate exports, and forwarding-only modules. Completed for sampling backend choice, descriptor metadata, the one-element registry, circuit path helpers, and empty facade forwarding files; broader compatibility-adapter deletion remains under task 5.
7. Keep consumer fixtures for direct component use, facade use, an external `CircuitPass`, a reusable `DecoderSession`, and SIMD-kernel isolation.
8. Update API, migration, architecture, and generated documentation in the same changes.

Execution-adapter checkpoint: `CompiledSampler`, `CompiledDetectionConverter`, `CompiledDemSampler`, their materialized or encoded return paths, callback visitors, facade-only helpers, and hidden engine bridges used only by those adapters are deleted. Sampling, measurement-to-detection conversion, and DEM sampling expose compilers, immutable plans, owned mutable sessions, typed sinks, and only the short-lived transactions needed to bind one sink across incremental writes and finalization. The obsolete whole-output budgets and public record DTOs are gone; canonical sessions retain bounded batch scratch while callers own output retention. Incremental conversion uses `MeasurementToDetectionTransaction`; incremental DEM replay uses owned `DemReplaySession` state plus `DemReplayTransaction`. Neither transaction is a compatibility route, and each makes cross-sink completion impossible by construction.

### Closeout Sequence

1. Complete: incremental DEM replay owns mutable state, while one short-lived transaction binds exactly one output sink for its lexical lifetime and releases that borrow when finalized or dropped.
2. Finish moving model, analysis, engine, and record semantics into tests owned by the crate that implements them; leave cross-component tests only where they prove an actual integration contract.
3. Bind every facade root re-export to its exact component source in the checked finite inventory and reject missing, duplicate, reordered, or wrong-owner entries.
4. Update direct-consumer fixtures, benchmark prerequisite ownership, migration guidance, component contracts, and generated ledgers from the final source graph.
5. Run owner tests first, then architecture checks, parity owners, oracle contracts, generated-contract checks, workspace formatting, warnings-denied Clippy, workspace tests, benchmark smoke, and pre-commit.
6. Run `milestone-audit` and `full-code-review` on the integrated diff, fix confirmed findings, and commit the source, generated contracts, and documentation in focused changes before selecting P3 work.

### Tests

- Run owner-crate semantic suites after each moved behavior.
- Compile and execute the supported external-consumer fixtures.
- Search for every removed public path and reject stale source, docs, examples, manifests, and generated entries.
- Verify that the facade and direct component route produce the same observable outputs on representative workflows.
- Require each parity family’s canonical semantic test to run in its declared product-owner package; `stab-oracle` is the only exception for independent pinned-source evidence.

### Benchmarks

- Preserve the pre-reset A4 and A5 work and output witnesses recorded from clean revision `18099bf3` in [Benchmark Contracts](../../benchmarks/README.md): A4 owns the sampling phase and generated-circuit CLI witnesses, while A5 owns detection, conversion, DEM phase, and eleven CLI witnesses.
- Run temporary diagnostic probes only when the P2 diff changes an active row's steady-state loop body, batch capacity, post-warmup allocation behavior, codec path, or process boundary. Pure package/test ownership moves and preflight or finalization checks do not trigger timing. For a triggered row, run that exact manifest ID through a fresh local baseline and compare and record the selector and outcome in this milestone's implementation checkpoint; do not promote the probe into the release matrix automatically.
- The integrated P2 changes preserve the existing execution loops, batch capacities, warmed storage, codec paths, and CLI process boundaries, so the source-current benchmark smoke plus the pre-reset A4 and A5 work/output witnesses satisfy this milestone. P7 remains responsible for the replacement user-visible timing system.

### Done Criteria

- Each algorithm has one production owner.
- The facade has no unique algorithm, duplicate model, or universal error type.
- No compatibility shim remains for a removed pre-1.0 Stab API.
- Public compilers return owned immutable plans and plans create owned mutable sessions. No reusable public plan or session stores a borrow into a caller-owned circuit, DEM, decoder, sink, or output buffer. A public incremental transaction may borrow one session and exactly one sink only for that transaction's lexical lifetime; it must not permit a different sink to receive finalization, survive forgotten lifecycle state as usable, or expose a second compatibility route.
- Every exported facade root item is identity-preserving and appears in the mechanically checked finite inventory; component operations remain under direct component namespaces.

### Implementation Checkpoint

- `stab-core` contains documentation, direct component namespaces, and a finite checked root re-export set. Facade-owned algorithms, duplicate models, the universal error, backend placeholders, forwarding modules, and legacy compiled adapters are removed.
- CLI and operational hot paths changed by P2 use owning crates directly. External consumer fixtures prove direct stable component use, scalar and nightly facade use, mixed feature unification, external circuit passes, decoder sessions, and SIMD isolation.
- Incremental measurement conversion and DEM replay bind one sink for one transaction lifetime. Abandoned committed transactions fail closed, reset rules are explicit, zero-shot behavior is defined, and cancellation reports cumulative committed progress.
- Semantic tests were moved to `stab-model`, `stab-analysis`, `stab-records`, and `stab-engine`; low-value derive and representation assertions were removed. The graph/vector simulator cross-check was deleted because those products remain deferred and the test duplicated owned algebra evidence.
- The facade checker binds every root export to its exact source crate and rejects crate-level behavioral attributes, local definitions, globs, wrong owners, duplicates, omissions, and reordering. The parity validator separately forbids `stab-core` from owning a semantic family.
- Formatting, warnings-denied workspace Clippy, all-feature workspace tests, architecture and consumer checks, 100 canonical parity owners, the live 62-case result-format corpus, 313 compatibility rows, 122 blocker cases, the implemented oracle suite, 48 runtime benchmark prerequisites, qualification status, and benchmark smoke pass on the integrated worktree.
- Milestone audit and full code review found no remaining implementation blocker or specification gap. The two revealed ambiguities, transaction sink lifetime and benchmark-probe triggering, are resolved in `milestone-spec-gaps.md`.
- P2 preserves active steady-state loops, batch capacities, warmed allocation behavior, codec paths, and CLI process boundaries. No diagnostic timing probe is triggered and no new performance claim is made; P7 owns replacement E2E evidence.

## Milestone P3: Close Model, Gate, And Record-Format Parity

### Tasks

1. Complete circuit and DEM parsing, canonical printing, tags, aliases, coordinate shifts, repeat blocks, targets, arguments, and diagnostics.
2. Cover every frozen nondeprecated gate, alias, legal argument family, and legal target shape from one canonical gate catalog.
3. Complete the six result codecs across dense, packed, sparse, typed DETS, streaming, side-output, and conversion modes where the representation applies.
4. Keep all text grammars byte-oriented and shared between materialized, streaming, replay, `convert`, and `m2d` consumers.
5. Document and test each deliberate resource-limit divergence.

### Tests

- Exact parser and printer fixtures for accepted syntax and canonical output.
- A malformed-byte differential corpus covering delimiters, whitespace, termination, overflow, bounds, namespaces, duplicates, and invalid bytes.
- Generated legal and illegal gate-shape tests from canonical metadata, with pinned-Stim comparison for each family.
- Property tests for parse-print-parse stability, folded structure preservation, codec equivalence, and streaming versus materialized results.
- Allocation and cancellation tests for record streaming at small, large, zero-width, and maximum accepted widths.

### Benchmarks

- E2E candidate probes for circuit canonicalization, dense `01`/`b8` conversion, and typed DETS conversion with observable side output.
- A candidate remains only if it represents a future user workflow or explains at least 10% of one workflow's measured time.

### Done Criteria

- Every model, gate, alias, target, and result-format parity row is `done`, `deferred`, or an explicit divergence.
- One grammar and one metadata catalog own each public syntax fact.
- No second CLI parser or duplicate format enum exists.

### Implementation Checkpoint

- The first P3 slice closes circuit and DEM approximate equality with one typed finite nonnegative `AbsoluteTolerance` and private iterative comparators over each model's reachable compact item tree. Numeric instruction arguments use inclusive absolute error; instruction kinds, gates, targets, argument counts, tags, repeat counts, ordering, and folded structure remain exact.
- Concise table-driven owners cover tolerance boundaries, probabilities, coordinates, shifts, separators, aliases, parser fusion, nested repeats, exact structural mismatches, invalid tolerances, and 4,096-level stack safety with both equal and unequal leaves.
- Two pinned Stim bugs are explicit divergences instead of copied behavior: exact and approximate circuit equality omit repeat tags, and removing a repeat through Python leaves hidden backing storage that can make visibly empty circuits unequal. Focused C++ probes linked to the frozen `libstim.a` reproduce both defects.
- Approximate equality receives no benchmark row because it is not an active release E2E workflow and no profile attributes at least 10% of a user workflow to it. This slice changes no existing measured loop and makes no timing claim.
- The second P3 slice adds `CircuitReferenceSigns`, `circuit_reference_signs`, and `circuit_reference_signs_with_limits` to `stab-engine`. The implementation reuses the engine's noiseless all-zero-sweep sampler and repeat-aware detection-conversion terms instead of adding a second simulator or annotation traversal in `stab-analysis`; configurable limits reuse the one detection-conversion policy.
- One semantic Rust owner covers Stim's documented result, detector declaration order, sparse observable ids, folded repeats, duplicate cancellation, ignored noise and measurement-flip probabilities, Pauli observable targets combined with `XCZ` and `YCZ` sweep controls, empty circuits, and typed invalid-record failures. The existing detection-resource suite separately proves exact admission boundaries. An independent C++ probe linked to the pinned `libstim.a` obtains the same signs through Stim's measurement-to-detection converter instead of copying Stab's implementation.
- Reference signs receive no benchmark row because the standalone API is not a release E2E workflow and no profile attributes at least 10% of one to its work. Existing sampling and detection workflows already own the shared hot paths.
- The final P3 slice extends `ParseLimits` with typed original-byte, compact-declaration, and retained-target admission. One private `ParseAdmission` owns both dialects: byte checks precede bounded byte-metadata preparation, declaration checks occur before payload parsing and circuit fusion, targets are admitted after decoding and before retention, model-item storage grows only after successful parsing, and wide fast paths obey the cumulative target budget. Byte preparation is bounded by source-byte and admitted-line policy and intentionally precedes semantic declaration and target admission.
- `ResourceLimitError` now stores dialect, resource, optional source line, values, and span directly; the multiplicative `ResourceLimitContext` enum and parser-only public `advanced` constructors are removed. `stab capabilities` schema version 5 exposes the complete default parser policy.
- One table-driven semantic owner covers exact reduced boundaries, first excess, folded declaration counting, circuit combiners, DEM separators, cumulative fast-path charging, pre-fusion declarations, zero limits, typed diagnostics, and byte-before-UTF-8 precedence through string and byte entry points. A separate allocation owner proves that a large suffix after a rejected or invalid command does not change allocation count or peak bytes and that blank or comment-only source reserves no model-item storage.
- Parser admission receives no benchmark row because rejection is not a release E2E workflow and no profile attributes at least 10% of one to these checks. Existing circuit and DEM parse workflows remain diagnostic performance owners.
- Removing input-derived top-level reservation triggered source-current diagnostics. A fresh `m4-circuit-parse` baseline and three-run compare under `target/benchmarks/p3-parser-admission-circuit-{baseline,compare}` measured direct dense and sparse ratios of `0.554x` and `0.503x`; serial sealed-worker probes measured `pq2-circuit-parse-adapter-smoke` at `0.922499x` and `pq2-dem-parse-adapter-smoke` at `0.915128x`. The initially concurrent adapter probes were discarded as contaminated. All values are dirty-tree diagnostics, not promotable evidence.
- P3 is complete: every P3-owned family is done, deferred, or an explicit divergence. The exact-boundary, optimization-independent-accounting, and rejected-source-allocation families are verified resource-limit divergences because Stim v1.16.0 has no equivalent configurable policy.

## Milestone P4: Close Sampling And Detection Parity

### Tasks

1. Execute every legal gate and target shape through the sampler.
2. Lower classical controls into one private typed execution representation.
3. Preserve folded repeats and support sweep and measurement-record feedback through nested repeat boundaries.
4. Use one correctness path for measurement sampling, reference comparison, detection conversion, and observable extraction.
5. Keep direct frame execution as an optimization and fall back to a fused valid execution path when a gate family lacks a specialized kernel.
6. Cover heralded and correlated errors, inversion, `MPAD`, `MPP`, pair measurements, resets, and noisy measure-reset families.
7. Keep exact Stim random streams out of scope while requiring deterministic semantic and statistical equivalence under Stab-owned seeds.

Execution checkpoint: ordinary sampling now uses the sole public sampling compiler for sweep-conditioned and non-sweep circuits. Legal `CX`, `CY`, `CZ`, `XCZ`, and `YCZ` sweep target orientations lower into the existing typed `SweepPauli` IR, omitted sweep data is all false through nested repeats, and compiler schema version 2 plus derived fingerprints identify the admission change. The generated classical-control semantic matrix owns the row across every accepted target pattern and real execution surface. No per-control benchmark was added because the existing release CLI sampling workflows already measure this lowering and execution architecture without creating a distinct user hot path.

Gate-surface checkpoint: a metadata-driven owner constructs and executes every declared legal canonical gate/target pattern through the public sampling plan and session, then executes a nested folded repeat. The owner deliberately proves admission and dispatch only; existing common, fixed-tableau, sweep, noise, and statistical owners retain value-level semantics. The audit found no missing legal sampler kernel, so no production branch or per-gate benchmark was added merely to close the aggregate ledger row.

### Tests

- Deterministic differential tests for every gate family and target orientation where noise is absent or forced to probability 0 or 1.
- Folded-versus-unrolled, batch-versus-shot, chunking, session reuse, and streaming-versus-materialized equivalence.
- Fixed-seed statistical tests for at least independent Pauli noise, correlated errors, heralded errors, reset/measurement noise, and DEM sampling, with predeclared tolerances and multiple-comparison handling.
- Sweep and record-feedback tests across nested repeats, including boundary indexes and invalid references.
- Cancellation, writer failure, zero-shot, wide-record, and bounded-buffer tests.

### Benchmarks

- Noisy surface-code sampling through the release CLI.
- Folded-repeat PTB64 sampling through the release CLI.
- Detection with observable output through the release CLI.
- Packed-sweep `m2d` through the release CLI.

### Done Criteria

- No legal in-scope gate or target reaches an unsupported execution error.
- All stochastic parity rows pass their predeclared semantic tests without RNG-stream identity claims.
- Sampling and detection use one compiled-plan/session architecture and one conversion plan.

## Milestone P5: Close Analysis, Transform, Search, And Algebra Parity

### Tasks

1. Support circuit-to-DEM lowering for every in-scope gate and target shape.
2. Complete loop folding, gauge handling, approximation controls, decomposition, correlated errors, coordinate propagation, and feedback across repeats.
3. Complete the selected public transforms: flattening, noise removal, error decomposition, simplification, feedback handling, inverse QEC construction, time reversal for flows, flow generation and checking, detecting regions, and missing-detector discovery.
4. Make the reverse tracker the sole implementation for reverse-flow state instead of preserving parallel representations.
5. Retain and independently prove graphlike shortest-error search, hypergraph heuristic search, shortest-error WCNF production, likeliest-error WCNF production, and detector-hyperedge semantics needed by analysis.
6. Keep reusable decoder sessions as a Stab extension with their own conformance tests and self-regression workflow; do not use them to close a Stim parity row.
7. Keep full `ErrorMatcher` provenance deferred rather than exposing a partial public contract.

### Tests

- Exact pinned-Stim comparison where output is canonical and semantic comparison where ordering is intentionally unconstrained.
- Folded-versus-unrolled and forward-versus-reverse cross-checks for repeats, feedback, coordinates, observables, and detector regions.
- Sparse-high-index, deeply nested, empty, zero-probability, high-observable, and resource-boundary cases.
- Search and satisfiability cross-checks against exhaustive small models and fixed independent witnesses.
- Extension conformance tests for decoder-session reset, reuse, dimension mismatch, deterministic decoding, and batch behavior.

### Benchmarks

- Folded circuit analysis through `analyze_errors`.
- A Stab-only reusable Rust sample-detect-decode workflow that covers the extension boundary and excludes one-time process startup.
- Diagnostic phase probes only for a profile-confirmed analysis or decoder bottleneck.

### Done Criteria

- Every nondeferred analysis, transform, search, and algebra row has one executable owner test; decoder conformance is reported separately as a Stab extension.
- Public APIs do not expose knowingly selected subsets as though they were complete.
- No duplicate reverse-tracking or DEM traversal representation remains.

## Milestone P6: Close CLI And Rust Workflow Parity

### Tasks

1. Complete the seven computational commands and help for all nondeprecated Stim-visible arguments and combinations in scope.
2. Remove deprecated top-level mode flags, hidden compatibility aliases, redundant routing, and unsupported flags that only advertise a future implementation.
3. Keep `.stim -> .stim` canonical conversion, Stab-native help text, and agent-native inspection commands as documented Stab additions or divergences, not Stim parity claims.
4. Expose idiomatic Rust workflows through direct component crates and the thin facade without recreating Python-shaped APIs.
5. Keep command-wide typed file-role validation before truncation for every path-bearing command.

### Tests

- Run the built release binary for success, failure, help, input/output paths, standard streams, side outputs, broken pipes, and exit status.
- Compare exact stdout for deterministic commands and semantic or statistical output for stochastic commands.
- Compare error class, nonzero status, and relevant flag names for invalid use without freezing incidental prose.
- Retain the complete direct, normalized, symlink, and hardlink alias matrix for destructive path combinations.
- Exercise large streaming inputs and outputs through actual files and pipes.

### Benchmarks

- Candidate command workflows are measured process-to-process against pinned Stim with identical inputs and equivalent output sinks.
- Individual flags and format pairs do not receive separate release benchmarks unless they produce a distinct hot path visible in profiles.

### Done Criteria

- Every in-scope CLI ledger entry is implemented and tested through the real binary.
- Help advertises only implemented or explicitly identified Stab-native behavior.
- No deprecated dispatch or duplicate argument-normalization path remains.

## Milestone P7: Replace Performance Machinery With One E2E Suite

### Tasks

1. Introduce `benchmarks/suite.toml` as the only active workload, parity, memory, and regression policy.
2. Implement one Rust benchmark runner in `ops/bench` using the existing bounded process supervisor.
3. Remove the legacy manifest, milestone threshold files, waivers, qualification runtime-group inventory, receipt trees, rollups, and completion workflow after the replacement reproduces their still-relevant conclusions.
4. Cap the release suite at 12 workflow families and 30 family-scale cases.
5. Cap persistent diagnostic probes at 15; adding one requires removing or consolidating another unless the cap itself is deliberately revised.

### Release Matrix

| Family | User workflow | Cases | Comparator | Parity prerequisites |
|---|---|---:|---|---|
| `generate-surface` | Generate and serialize rotated surface-code circuits | 2 | Stim CLI | `generation.surface-code-memory`, `generation.parameters-and-noise`, `cli.gen` |
| `convert-sparse` | Convert `hits` and `r8` records in practical sparse-width classes | 2 | Stim CLI | `result-formats.codecs-and-strict-grammars`, `result-formats.routes-convert`, `cli.convert` |
| `convert-dense` | Convert dense `01` and `b8` records in both practical width classes | 2 | Stim CLI | `result-formats.codecs-and-strict-grammars`, `result-formats.routes-convert`, `cli.convert` |
| `convert-typed-dets` | Convert detector and observable records with side output | 2 | Stim CLI | `result-formats.codecs-and-strict-grammars`, `result-formats.streaming-and-typed-dets`, `result-formats.routes-convert`, `cli.convert` |
| `sample-surface` | Sample noisy surface-code circuits | 3 | Stim CLI | `sampling.circuit-common-measurement-and-reset`, `sampling.circuit-core-gates`, `sampling.noise-complete`, `result-formats.routes-sample`, `cli.sample` |
| `sample-folded-ptb64` | Sample compact repeated circuits to PTB64 | 2 | Stim CLI | `circuit-model.syntax-and-canonical-text`, `sampling.loop-folding-selection`, `result-formats.routes-sample`, `cli.sample` |
| `detect-observables` | Sample detections and observables from noisy circuits | 3 | Stim CLI | `detection.common-frame-gate-surface`, `detection.reference-correction`, `result-formats.routes-detect`, `cli.detect` |
| `m2d-packed-sweep` | Convert packed measurements and sweep data to detections | 3 | Stim CLI | `detection.reference-correction`, `detection.sweep-and-feedback-common`, `result-formats.routes-m2d`, `cli.m2d` |
| `analyze-folded` | Convert folded circuits to detector error models | 3 | Stim CLI | `analysis.circuit-to-dem-selected-gate-surface`, `analysis.loop-folding-common`, `cli.analyze-errors` |
| `sample-dem` | Sample repeated and sparse detector error models | 3 | Stim CLI | `dem-model.syntax-and-canonical-text`, `sampling.dem-sampling-and-replay`, `result-formats.routes-sample-dem`, `result-formats.routes-sample-dem-replay`, `cli.sample-dem` |
| `qec-cli-pipeline` | Generate, analyze, then sample a detector error model | 2 | Stim CLI pipeline | `generation.surface-code-memory`, `analysis.circuit-to-dem-selected-gate-surface`, `sampling.dem-sampling-and-replay`, `cli.gen`, `cli.analyze-errors`, `cli.sample-dem` |
| `qec-rust-pipeline` | Reuse compiled sample, detect, and decode sessions | 2 | Stab self-regression only | `sampling.circuit-common-measurement-and-reset`, `sampling.circuit-core-gates`, `detection.common-frame-gate-surface`; decoder session is a separately tested Stab extension |

The initial matrix contains 29 family-scale cases. Exact arguments, deterministic inputs, semantic work units, expected outputs, and scale labels live only in `benchmarks/suite.toml`.

### Diagnostic Budget

The initial diagnostic candidates are circuit parse, circuit print, DEM parse, DEM print, sampler compile, sampler execute, detection conversion, analyzer lowering, record codec, sparse XOR, bit transpose, tableau composition, and Clifford composition. A diagnostic persists only when a profile shows that it accounts for at least 10% of an E2E workflow or when it is needed to isolate a confirmed regression. Gate lookup, singleton getters, trivial formatting, fingerprints, and protocol plumbing are not benchmark products.

### Measurement Contract

- Benchmark actual release binaries for CLI families and the stable public Rust workflow for the library family.
- Use byte-identical deterministic inputs, equivalent arguments, fully drained outputs, and output validation outside the timed interval.
- Include startup, parsing, compilation, execution, conversion, encoding, and I/O in CLI E2E timing.
- Alternate Stim and Stab deterministically, retain all samples, use paired semantic-work ratios, and report the paired median with a fixed-seed bootstrap confidence interval.
- Report wall time, semantic throughput, peak RSS, and output size. Keep startup as a diagnostic field rather than subtracting it from CLI timing.
- Enforce Stim parity at median and confidence-interval upper bound `<= 1.25x`.
- Enforce Stab self-regression at median and confidence-interval upper bound `<= 1.15x` against an accepted architecture-specific baseline.
- Treat missing or identity-mismatched self baselines as `unseeded`, never passing.
- Do not add timing waivers or relax thresholds to accept a regression.

### Tests

- Reject duplicate family/scale IDs, more than 12 families, more than 30 release cases, more than 15 diagnostics, missing correctness prerequisites, missing comparators, and inconsistent semantic work.
- Verify exact command arguments, input digests, output comparators, complete output consumption, paired ordering, fixed bootstrap replay, and unseeded self-regression behavior.
- Preserve adversarial process-supervisor tests and add deterministic offline replay of all report calculations.

### Done Criteria

- One runner and one manifest own active performance truth.
- Every release case represents an actual CLI or Rust user workflow.
- Microbenchmarks are profile-justified diagnostics, not release claims.
- The replacement suite reproduces or supersedes every still-relevant current timing and memory conclusion before the old system is deleted.

## Milestone P8: Optimize User-Visible Regressions

### Tasks

For each failing E2E case:

1. Reproduce the regression with validated semantic work and outputs.
2. Profile the full workflow and attribute cost to startup, parse, compile, execute, convert, encode, allocation, or I/O.
3. Add a temporary focused probe for the dominant phase if no existing diagnostic isolates it.
4. Optimize the canonical owner, preserving the simple scalar or semantic reference when useful.
5. Re-run owner correctness, pinned differential checks, E2E timing, and peak RSS.
6. Remove the temporary probe unless it satisfies the persistent diagnostic rule.

### Tests

- Every optimization keeps the owning semantic tests and affected differential rows green.
- SIMD changes compare scalar and SIMD implementations across alignment, tails, zero width, maximum width, and randomized fixed-seed inputs.
- Allocation changes retain explicit cancellation, output-error, and resource-boundary tests.

### Benchmarks

- Use the failing E2E family as the acceptance benchmark.
- A diagnostic result may explain a change but cannot replace the E2E result.
- Compare peak RSS and scaling whenever an optimization changes buffering, batching, or representation.

### Done Criteria

- Every comparable release case satisfies the unchanged `1.25x` Stim parity gate.
- Every seeded case satisfies the unchanged `1.15x` Stab self-regression gate.
- No waiver, hidden workload reduction, or duplicate representation was introduced to obtain the result.

## Milestone P9: Produce One Formal Evidence Bundle And Retire History

### Tasks

1. Run the complete deterministic correctness suite from one clean committed revision.
2. Run full and soak E2E evidence on the controlled AArch64 host.
3. Require fixed toolchains and build flags, CPU affinity, no competing benchmark process, host temperature below `100 C`, and no swap I/O during measured samples.
4. Restore the exact prior swap configuration after the run, including failure paths.
5. Publish one immutable run bundle containing `run.json`, raw samples, derived report, correctness result, host profile, source and Stim identities, toolchain and build identities, input and output digests, accepted self-baseline identities, memory results, and offline replay metadata.
6. Maintain one current AArch64 evidence pointer. Keep prior bundles and failed runs as Git or artifact history instead of copying their state into current documents.
7. Seed the first AArch64 Stab self-regression baseline from the accepted full and soak samples; the seeding run proves parity but is `unseeded` for self-regression.
8. Leave x86-64 explicitly unqualified until a controlled host is available; this is not a blocker for the selected AArch64 release claim.
9. Extract durable decisions into architecture records, then delete superseded plans, progress reports, dashboards, ledgers, old benchmark schemas, and obsolete operational commands.
10. Leave `docs/plans/` with this plan, the short `GOAL.md`, and only genuinely active future plans.
11. Run `milestone-audit` and `full-code-review`, fix confirmed findings, and regenerate the parity view from the final source.

### Tests And Verification

- Replay the bundle offline and require byte-identical derived results.
- Reject dirty source, wrong Stim identity, mismatched inputs, existing output paths, incomplete samples, failed correctness prerequisites, altered thresholds, and unseeded regression claims.
- Run formatting, workspace Clippy, workspace tests, parity check and rendering, the full pinned oracle tier, E2E manifest validation, E2E smoke, documentation checks, and pre-commit.

### Done Criteria

- The AArch64 evidence bundle replays from raw samples and exact identities.
- Public documentation contains no stale status count or superseded qualification procedure.
- The worktree is clean, no benchmark process remains, swap state matches its pre-run state, and CI passes the exact source commit.
- The selected core Rust and CLI ledger has no `missing` entry.

## Test Suite Contract

The final suite has five layers:

1. **Owner tests:** deterministic semantic tests in the crate that owns the behavior.
2. **Pinned differential tests:** compact generated and curated comparisons against Stim v1.16.0 for compatibility-sensitive behavior.
3. **Property and metamorphic tests:** fixed-seed invariants such as round trips, folded/unrolled equivalence, algebraic identities, and chunking invariance.
4. **Real CLI tests:** process-level workflows covering arguments, files, streams, outputs, errors, and exit status.
5. **Focused safety tests:** hostile input, path identity, process supervision, allocation bounds, cancellation, and writer failure.

Pull-request CI runs deterministic owner tests, the compact parity suite, architecture checks, CLI smoke, generated-document checks, and untimed benchmark preflight. Nightly CI runs the full differential, fuzz, and statistical tiers. Shared-host timing is diagnostic only. Formal performance evidence runs only on the controlled host.

Tests that merely assert derives, re-exports, constants, type names, private pointer identity, static labels, or bookkeeping shape are removed unless that representation is itself a documented public contract.

## Operational Commands

The target human-facing command surface is:

```text
just oracle::parity-check
just oracle::parity-run --tier pr|full|soak
just oracle::parity-render --check
just bench::e2e-check
just bench::e2e-run --tier smoke|full|soak
just bench::e2e-replay --input <bundle>
```

Complex logic remains in Rust operations binaries. The old qualification and benchmark commands are deleted only after these replacements pass and reproduce their durable conclusions.

## Documentation Contract

- `GOAL.md` stays below 80 lines and contains only current state, current milestone, blockers, next commands, non-negotiable gates, and links to active sources.
- This plan owns the execution sequence and rationale.
- `oracle/stim-v1.16-parity.toml` owns feature status.
- Generated `docs/stim-parity.md` presents that status without independent prose counts.
- `benchmarks/suite.toml` owns active workloads and thresholds.
- Architecture records own durable design decisions.
- Historical progress reports are not active specifications and are deleted at P9 after durable rationale is retained.

## Final Acceptance

The program is complete when every nondeprecated, nondeferred core Rust and CLI behavior in the frozen Stim v1.16.0 ledger is `done` or an approved bug/resource divergence; every surviving test protects meaningful behavior; the architecture has one owner and one public route per capability; the 29-case E2E suite is the only active performance system; controlled AArch64 evidence passes correctness, `1.25x` Stim parity, memory, and seeded `1.15x` self-regression where a prior baseline exists; the evidence bundle replays offline; and superseded compatibility and qualification machinery has been deleted.

## Assumptions

- Stim v1.16.0 remains frozen for this program.
- Breaking obsolete Stab APIs before 1.0 is acceptable and receives migration notes rather than shims.
- Safe resource-limit divergences are acceptable when typed, documented, and tested.
- Exact Stim random streams are not required.
- The release claim is controlled AArch64 first; controlled x86-64 evidence follows later.
- Development remains directly on `main`; no branch or linked worktree is created unless the user changes that policy.
