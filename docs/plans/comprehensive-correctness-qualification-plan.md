# Comprehensive Correctness Qualification Plan

## Status

Planned: 2026-07-13.

Compatibility target: Stim v1.16.0 at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07` in `vendor/stim`.

Scope target: every implemented, non-deferred Rust and CLI contract identified by `docs/stab-feature-checklist.md`, including every exported Rust API item that implements those selected contracts.

## Objective

Build a source-owned correctness qualification suite that can answer, for every selected Stab feature, exactly what behavior is covered, which pinned Stim test or source contract owns the expectation, which comparator is used, which negative and resource boundaries are exercised, and which command reproduces the evidence.

The suite must test Stab against pinned Stim where a public or internal equivalent exists and must use independently justified semantic, property, or resource evidence where byte identity is not the contract.
It must not treat whole upstream files, broad Cargo selectors, nearby tests, or an all-green workspace run as proof that a feature is comprehensively covered.

## Meaning Of Comprehensive

The correctness suite is comprehensive only when all of the following are true:

1. Every implemented selected feature and exported Rust API item has a stable qualification id or explicit parent mapping and at least one independently selectable primary case.
2. Every relevant C++ GTest or Python semantic test in the selected upstream files has a case-level disposition of `exact-oracle`, `ported-rust`, `semantic-mining`, `deferred-product`, `not-applicable`, or `superseded`, with a non-empty reason for every non-executable disposition.
3. Every public parser, input, output, path, replay, conversion, sampling, and generation boundary has positive, negative, and resource evidence.
4. Every probabilistic claim has a source-owned statistical plan with fixed shots, seeds, buckets, expected probabilities or a declared two-sample model, an effect-size target, an exact acceptance rule, and a familywise false-positive budget.
5. Every supported CLI command has evidence for accepted flags, rejected flags, stdin, stdout, path IO, stderr class, exit status, writer failure, and applicable format or width matrices.
6. Every materialized API or inherently expanded output has a tested cap, and every streaming API has tests for bounded buffering, early visitor or writer failure, and record-count or width errors.
7. Every explicit deferral remains visible and cannot be counted as a passing selected case.
8. The full qualification report has no missing, shared-primary, stale, ambiguous, or unowned case.

Comprehensive does not mean implementing deferred Python, JS/WASM, diagram, ecosystem, public interactive simulator, full ErrorMatcher provenance, exact random-stream, C++ header compatibility, QASM, Quirk, Crumble, or GPU products.

## Sources Of Truth

- Feature boundary: `docs/stab-feature-checklist.md`.
- Upstream inventory: `docs/stim-feature-list.md` and `docs/plans/stim-test-porting-plan.md`.
- Frozen implementation evidence: `oracle/compatibility-matrix.csv`, `oracle/fixtures/manifest.csv`, and `docs/plans/blocker-closure-ledger.json`.
- Existing Rust tests: workspace unit and integration tests under `crates/` and `ops/`.
- Generated Rust public API inventory: rustdoc JSON from the pinned Nightly toolchain for selected workspace crates.
- Frozen upstream implementation and tests: `vendor/stim`.
- Planning lessons: `docs/plans/lessons-learned.md`.
- Performance coupling: `docs/plans/comprehensive-stim-performance-qualification-plan.md`.

If these sources disagree, the qualification inventory must record the disagreement and the owning documentation must be fixed before the affected case can pass.

## Planned Artifacts And Commands

### Machine-Readable Inventory

Add `oracle/qualification-manifest.json` as the source-owned case inventory.
Do not duplicate fixture payloads, benchmark rows, or blocker-ledger details inside it; reference their stable ids and reject missing or stale references.

Each case must include:

- `id`: stable qualification case id.
- `feature_id`: stable feature id from the domain matrix in this plan.
- `public_api_items`: exact rustdoc paths for exported Rust items owned by the case, empty only for non-Rust surfaces.
- `surface`: Rust API, CLI, file format, engine, or resource boundary under test.
- `upstream`: exact path, provenance kind, complete test or source symbol, subcase, and typed gate markers where relevant.
- `disposition`: `exact-oracle`, `ported-rust`, `semantic-mining`, `deferred-product`, `not-applicable`, or `superseded`.
- `comparator`: `exact-bytes`, `exact-value`, `canonical`, `error-class`, `structural`, `state-equivalence`, `semantic-invariant`, `statistical`, `property`, or `resource`.
- `primary_selector`: one exact Cargo test, oracle fixture id, property target, or ops check that selects only this case.
- `supporting_selectors`: optional shared evidence that cannot replace the primary selector.
- `statistical_plan`: required only for statistical cases and bound to executable core data.
- `resource_contract`: `streaming`, `bounded-materialized`, `bounded-search`, `constant-scratch`, or `not-applicable`, plus explicit limits or slope rules.
- `negative_axes`: named malformed, unsupported, overflow, path, width, count, or writer-failure cases.
- `performance_groups`: benchmark qualification groups whose timed workloads depend on this case.
- `status`: `planned`, `implemented`, `evidence-close`, or `deferred`.

The manifest schema must deny unknown fields, enforce bounded row and string counts before expensive work, reject unsafe paths and symlinks, validate exact upstream anchors inside pinned files, and include a frozen semantic digest.

### Operational Surface

Extend `stab-oracle` instead of adding shell scripts.
Expose thin recipes from a new modular `justfiles/qualification.just` file:

```sh
just qualification::correctness-list
just qualification::correctness-check
just qualification::correctness-run --tier pr
just qualification::correctness-run --tier full
just qualification::correctness-run --tier soak
just qualification::correctness-report --out target/qualification/correctness/latest
```

Complex selection, source-anchor validation, subprocess execution, timeout handling, statistical evaluation, and report generation belong in Rust under `ops/oracle`.

### Report Contract

The generated JSON and Markdown reports must record:

- Stab commit and `local_modifications`.
- Stim tag and commit.
- Rust toolchain, target triple, operating system, and architecture.
- Selected tier, feature filters, seeds, shot totals, and property corpus ids.
- Passed, failed, deferred, semantic-mining, and not-applicable counts by domain and comparator.
- Every failed case with its exact primary selector and artifact paths.
- Every deferred case with its named deferred product.
- Suite-wide statistical false-positive budget and consumed bound.
- Resource caps and scaling checks exercised during the run.

## Comparator Policy

### Exact And Canonical

- Use `exact-bytes` for public CLI stdout, side-output bytes, canonical files, and result formats whose bytes are contractual.
- Use `exact-value` for typed Rust values whose ordering and representation are contractual.
- Use `canonical` when both implementations may accept multiple equivalent inputs but must print or normalize to the same canonical form.
- Do not normalize away differences in record ordering, detector ids, observable ids, tags, coordinate shifts, stderr class, exit status, or accepted flags unless the case explicitly declares those fields non-contractual.

### Structural And Semantic

- Use `structural` for graphs, DEMs, flows, or search results where order or tie choice is non-contractual but counts, minimum weight, target signatures, declarations, and memberships are contractual.
- Use `state-equivalence` for Clifford or Pauli transformations and require tests on a separating set of stabilizer states or tableaus, not only gate-plus-inverse cancellation on `|0>`.
- Use `semantic-invariant` only when the invariant is named and demonstrates a non-vacuous effect, including positive, no-op, and rejection cases where applicable.
- Any custom structural comparator must have its own adversarial tests proving that it rejects a missing, extra, reordered-contractual, wrong-weight, wrong-sign, or wrong-target result.

### Statistical

- Exact random-stream equality is not required.
- Prefer comparison to an analytically known categorical or Bernoulli distribution.
- Use a two-sample Stim-versus-Stab test only when the expected distribution cannot be expressed directly, and predeclare the minimum detectable effect and power.
- Every case must declare fixed seeds, shot count, buckets, acceptance boundaries, and familywise budget in machine-readable data shared with the executable test.
- Individual cases must have a false-positive budget no greater than `1e-6`, and the full selected suite must have a summed familywise bound no greater than `1e-4`.
- The validator and executable test must share canonical integer count boundaries so floating-point equality cannot admit a weaker validation plan.
- A failed statistical seed is a failure; diagnostic reruns may add evidence but may not replace the failed result.
- The `full` tier uses the frozen primary seed set, while the `soak` tier adds a deterministic seed panel and reports every seed.

### Property And Metamorphic

- Every property case must use a source-owned generator domain, maximum size, deterministic seed, and case count.
- Persist minimized regressions as ordinary focused tests or committed corpus inputs.
- Use pinned Stim as the differential oracle for bounded generated inputs when practical.
- Required metamorphic relations include parse-print-parse stability, accepted result-format round trips, folded-versus-unrolled equivalence, transform preservation, algebra group laws, sparse-versus-dense agreement, and search agreement with brute force on small models.

### Negative And Resource

- Match error class, command exit status, stderr emptiness class, and whether output files were created or partially written.
- Test exact cap, cap plus one, arithmetic overflow, excessive nesting, excessive record width, excessive shot count where materialized, visitor or writer error, malformed trailing data, unsafe path components, symlink policy, missing files, and broken pipes where the surface can encounter them.
- Resource evidence must measure the claimed property directly; allocation tests do not prove absence of copying, semantic tests do not prove bounded memory, and a timeout does not prove an early admission check.

## Qualification Tiers

| Tier | Intended use | Required contents | Target behavior |
| --- | --- | --- | --- |
| `schema` | Every local invocation | Manifest, anchor, selector, digest, deferral, and statistical-plan validation | No production execution |
| `pr` | Pull requests and pre-merge | Deterministic exact cases, focused negative cases, bounded property corpus, low-cost resource checks, one frozen statistical seed where affordable | Stable and reasonably short |
| `full` | Nightly and release candidates | Every implemented case, complete CLI matrices, full property corpora, full frozen statistical plans, resource boundaries, and selected fuzz corpus replay | Authoritative correctness qualification |
| `soak` | Scheduled deep validation | Multi-seed statistical panels, long fuzzing, large generated circuits and DEMs, allocator and RSS scaling probes, repeated writer failures, and platform matrix | Diagnostic depth without weakening `full` |

No case may exist only in `soak` if it protects deterministic public behavior that can regress in an ordinary change.

## Domain Matrix

| Feature id | Selected surface | Required correctness axes | Primary upstream sources | Performance groups |
| --- | --- | --- | --- | --- |
| `CQ-STIM-FORMAT` | `.stim` parser, validator, aliases, tags, targets, repeats, canonical printer | Exact and canonical corpus, AST properties, malformed syntax, nesting and line limits, round trips, all target kinds | `circuit.test.cc`, `circuit_instruction.test.cc`, `gate_target.test.cc` | `PERF-CIRCUIT-MODEL` |
| `CQ-DEM-FORMAT` | `.dem` parser, printer, instructions, shifts, coordinates, repeats | Exact bytes, canonical round trips, malformed targets and probabilities, overflow, folded traversal, selected coordinate lookup, materialization caps | `dem_instruction.test.cc`, `detector_error_model.test.cc` | `PERF-DEM-MODEL` |
| `CQ-RESULT-FORMATS` | `01`, `b8`, `r8`, `hits`, `dets`, `ptb64` readers, writers, and accepted conversion pairs | Exact bytes, all accepted round trips, padding, group-of-64 behavior, sparse indices, truncation, malformed records, width and count limits | `measure_record*.test.cc`, `sparse_shot.test.cc`, `command_convert.test.cc` | `PERF-RESULT-IO`, `PERF-CONVERT-CLI` |
| `CQ-GATE-CONTRACT` | 81 canonical gates, aliases, metadata, validation, and selected eight-surface execution | Registry exactness, typed targets, separating-state semantics, deterministic and statistical gates, no-op and rejection cases, cross-engine consistency | `gates.test.cc`, `frame_simulator.test.cc`, `tableau_simulator.test.cc`, `error_analyzer.test.cc` | `PERF-GATE-CONTRACT`, `PERF-SAMPLING`, `PERF-DETECTION`, `PERF-ERROR-ANALYSIS` |
| `CQ-BIT-KERNELS` | Scalar and portable-SIMD dense bits, bit tables, transposition, parity, random fill, and sparse XOR helpers used by selected engines | Scalar differential checks, unaligned tails, empty and maximum widths, overlap policy, sparse-density crossover, deterministic random-fill contracts, allocation and mutation boundaries | SIMD bit, bit-table, word, sparse-XOR, and probability utility GTests | `PERF-BIT-KERNELS` |
| `CQ-CIRCUIT-API` | Construction, mutation, introspection, coordinates, repeat handling, and selected transforms | Exact values, round trips, clone and mutation independence, folded-versus-unrolled properties, transform invariants, caps and unsupported shapes | `circuit.test.cc`, `circuit_pybind_test.py`, transform and inverse tests | `PERF-CIRCUIT-MODEL`, `PERF-FLOWS-AND-DETECTOR-UTILITIES` |
| `CQ-GENERATION` | Repetition, rotated and unrotated surface, color-code generation and noise knobs | Exact small goldens, structural large cases, parameter errors, deterministic output, detector and observable validity | generator C++ tests and `command_gen.test.cc` | `PERF-GENERATION` |
| `CQ-ALGEBRA` | Pauli, Clifford, Tableau, Flow, iterators, conversions, and stabilizer solving | Exact algebra examples, group laws, commutation, conjugation, inverse, iterator cardinality, scalar cross-checks, invalid shapes | stabilizer GTests and selected util-top tests | `PERF-STABILIZER-ALGEBRA` |
| `CQ-SAMPLING` | Compiled measurement sampling, reference samples, frame and tableau paths, supported noise, repeats, herald records | Exact deterministic records, statistical distributions, separating circuits, skip-reference and loop-folding behavior, streaming writers, seed semantics | sampler GTests and `command_sample.test.cc` | `PERF-SAMPLING` |
| `CQ-DETECTION` | Detection conversion, detector-frame sampling, `detect`, and `m2d` including selected sweep and feedback | Exact detector and observable records, reference subtraction, format matrices, sweep defaults, feedback direction, frame selection, streaming and writer failure | measurement-to-detection, frame simulator, detect, and m2d tests | `PERF-DETECTION` |
| `CQ-DEM-SAMPLING` | Compiled DEM sampling, replay, sampled-error records, observables, repeats | Exact deterministic cases, statistical noise, replay equivalence, sparse and dense models, folded repeats, streaming, materialized caps | `dem_sampler.test.cc`, `command_sample_dem.test.cc` | `PERF-DEM-SAMPLING` |
| `CQ-ANALYZER` | Circuit-to-DEM analysis, gauge handling, decomposition, approximation, loop folding | Exact DEMs, folded-versus-unrolled properties, generated codes, statistical channel semantics, diagnostics, admission caps | `error_analyzer.test.cc`, `command_analyze_errors.test.cc` | `PERF-ERROR-ANALYSIS` |
| `CQ-SEARCH` | Graphlike, hypergraph, shortest errors, SAT/WCNF, and selected matcher filtering | Exact small models, structural minimum-weight parity, brute-force differential, tie policy, zero probability, sparse ids, repeat traversal, independent caps | graphlike, hypergraph, WCNF, and matcher tests | `PERF-SEARCH-AND-MATCHING` |
| `CQ-FLOW-UTILS` | Flow generation, checking, solving, detecting regions, missing detectors, reverse tracking, inverse and feedback utilities | Exact pinned cases, generated differential cases, solver properties, signed and unsigned distinctions, repeat and high-index resource behavior | util-top flow, detecting-region, missing-detector, inverse, and feedback tests | `PERF-FLOWS-AND-DETECTOR-UTILITIES` |
| `CQ-CLI` | `gen`, `convert`, `sample`, `detect`, `m2d`, `analyze_errors`, `sample_dem`, help, and selected legacy dispatch | Accepted and rejected flags, default values, conflicts, stdin/stdout, path precedence, side outputs, exact bytes, stderr class, exit status, broken pipe | all supported command tests and namespaced main tests | All applicable `PERF-*` process CLI groups |
| `CQ-RESOURCE` | Cross-cutting hostile inputs and memory behavior | Exact admission boundaries, bounded buffers, traversal work, allocation slopes, writer and visitor failure, path and symlink policy | Existing Stab resource tests plus pinned behavioral anchors | `PERF-RESOURCE-BOUNDARIES` and every measured scale family |

Every row in this table must expand into independently selectable cases in `oracle/qualification-manifest.json`; a single umbrella case cannot close a domain.

## Milestone CQ0: Freeze The Case-Level Inventory

### Objective

Convert the current file-level test hierarchy and feature checklist into a finite case-level qualification inventory before adding new tests.

### Tasks

- Parse all GTest macro declarations from the 103 pinned C++ test files and all pytest function declarations from the 91 pinned Python test files.
- Generate a deterministic rustdoc JSON inventory for selected `stab-core` and `stab-cli` exports and assign every public item to a qualification feature, explicit parent contract, or documented non-semantic API disposition.
- Define the public API filter as default-feature reachable types, functions, constants, inherent methods, and explicitly implemented public traits; exclude compiler-generated auto or blanket implementation noise and the evidence-only `ops-contracts` feature from product API counts while testing that evidence-only exports do not leak into default builds.
- Record the exact extraction grammar, including `TEST`, `TEST_F`, `TYPED_TEST`, and Stim word-size macros; reject ambiguous or duplicate anchors.
- Classify every upstream case in a file that contributes to an implemented selected surface.
- Import existing blocker-ledger, oracle, and Cargo-test evidence by stable id without copying its content.
- Assign every selected feature one or more domain ids and primary qualification cases.
- Name all deferred-product and not-applicable cases explicitly.
- Add a semantic digest and bounded validation before source-file reads or statistical work.

### Tests

- Parser tests for every supported GTest macro form and pytest function form.
- Regressions for declaration prefixes, comments, strings containing fake declarations, duplicate names, one-character gates, longer gate-name substrings, and anchors present only in declarations.
- Manifest tests for duplicate ids, missing fields, unsafe paths, symlinks, unknown dispositions, stale evidence ids, shared primary selectors, unknown feature ids, and digest mismatch.
- Public API inventory tests for duplicate canonical paths, stale items, undocumented additions, re-exports, trait implementations, feature-gated exports, and items mapped to missing cases.
- A completeness test proving every selected feature id has an executable or explicitly evidence-closed case and every deferred case names its product.

### Acceptance Criteria

- `just qualification::correctness-check` reports exact counts by disposition and domain.
- No selected feature is represented only by a file-level row.
- No selected exported Rust API item is unclassified or represented only by module-level evidence.
- No implemented case has a shared or non-resolving primary selector.
- The inventory can be regenerated deterministically from pinned Stim and checked without modifying it.

## Milestone CQ1: Build The Qualification Harness

### Objective

Make every comparator, selector, statistical plan, artifact, timeout, and report contract executable before expanding feature tests.

### Tasks

- Add tier and feature filtering to `stab-oracle` qualification commands.
- Reuse existing exact fixture and Cargo-test execution paths.
- Add canonical, state-equivalence, semantic-invariant, property, and resource result adapters with typed outputs.
- Add suite-wide statistical-budget validation and deterministic seed-panel expansion.
- Add per-case subprocess timeouts, bounded stdout and stderr capture, and safe artifact paths under `target/qualification/`.
- Make report generation fail when a selected case is missing, deferred without selection permission, stale, unowned, or unexpectedly skipped.
- Add correctness preflight ids that the performance suite can require before timing a workload.

### Tests

- Comparator mutation tests that introduce one wrong byte, target, sign, weight, count, tag, coordinate, exit status, or error class.
- Statistical tests for integer boundaries, impossible plans, excess suite budget, missing buckets, duplicate seeds, and rerun-until-pass rejection.
- Property-runner tests for deterministic seeds, shrinking, persisted regressions, timeouts, and oversized generated cases.
- Report tests for dirty worktrees, wrong Stim commit, missing selectors, partial runs, artifact traversal attempts, and non-UTF-8 subprocess output.

### Acceptance Criteria

- All comparator classes have adversarial unit tests.
- A deliberately weakened or missing case makes the suite fail.
- Reports are reproducible, bounded, and identify exact commits and selectors.
- Performance workloads can name and validate correctness prerequisites without parsing Markdown.

## Milestone CQ2: Deterministic Models, Formats, Generation, And Algebra

### Objective

Complete deterministic and property coverage for `CQ-STIM-FORMAT`, `CQ-DEM-FORMAT`, `CQ-RESULT-FORMATS`, `CQ-GATE-CONTRACT`, `CQ-BIT-KERNELS`, `CQ-CIRCUIT-API`, `CQ-GENERATION`, and `CQ-ALGEBRA`.

### Tasks

- Port every relevant deterministic upstream subcase, splitting aggregated tests at distinct behavioral anchors.
- Generate accepted result-format conversion matrices and explicit rejected matrices from typed format capabilities.
- Add bounded parser and printer differential corpora covering empty, minimal, representative, nested, wide, sparse, tagged, shifted, malformed, and overflow shapes.
- Add separating-state fixed-tableau tests, metadata exactness, alias coverage, and target-role rejection tests for all gates.
- Add scalar-versus-SIMD and dense-versus-sparse differential corpora across zero length, unaligned tails, word and lane boundaries, sparse-density transitions, and large widths.
- Add circuit and DEM folded-versus-unrolled properties and transform-preservation properties.
- Add exact small generator goldens and structural larger generated-code checks.
- Add algebra laws and scalar reference comparisons over boundary sizes around SIMD word widths.

### Tests

- Exact pinned fixtures for canonical `.stim`, `.dem`, generated circuits, and each result format.
- Property cases for parse-print-parse, format round trips, repeat folding, transform idempotence where contractual, Pauli and Tableau laws, and sparse-versus-dense agreement.
- Negative cases for malformed syntax, invalid probabilities, bad target grouping, non-finite coordinates, count overflow, partial `ptb64` groups, invalid sparse indices, and excessive materialization.
- Allocation and work-boundary cases for parser lines, nesting, result widths, flattening, coordinate maps, and generator output admission.

### Acceptance Criteria

- Every owned deterministic upstream case has an exact disposition and selector.
- All accepted format pairs round trip and all rejected pairs fail with the owned error class.
- Gate metadata and semantics have no unknown state.
- Property corpora are deterministic and minimized regressions are persisted.

## Milestone CQ3: Public CLI Contract Matrix

### Objective

Qualify every implemented command as a public process contract instead of inferring CLI parity from core tests.

### Tasks

- Build typed flag and format matrices for all eight implemented command families.
- Port supported command GTest subcases at the flag-combination level.
- Exercise stdin, stdout, regular input and output paths, observable and error side outputs, empty files, missing files, pre-existing output files, writer failure, and path-error precedence.
- Validate selected legacy aliases and multiple-mode conflicts.
- Keep Stab-native help structural rather than claiming exact Stim prose.
- Test deprecated and deferred command spellings as explicit rejections or absent help topics.

### Tests

- Exact byte fixtures for deterministic commands and every public result format where accepted.
- Structural cases for help topics and generated outputs whose formatting is intentionally Stab-native.
- Statistical command cases for `sample` and `sample_dem` using shared plans.
- Negative cases for unknown flags, missing values, conflicting modes, invalid formats, width mismatch, partial records, malformed circuits or DEMs, missing paths, unwritable outputs, broken pipes, and unsupported deprecated modes.
- Streaming resource cases proving huge shot counts fail only on the injected writer when no materialization is required.

### Acceptance Criteria

- Every implemented CLI option is covered by at least one positive case and every rejected option class by a negative case.
- Core-only evidence is not used as the primary selector for a CLI case.
- Output creation and error precedence match the pinned command contract where selected.
- No deferred command is listed as implemented.

## Milestone CQ4: Sampling, Detection, DEM Sampling, And Statistical Semantics

### Objective

Qualify every implemented probabilistic engine and its deterministic reference behavior without requiring exact Stim random streams.

### Tasks

- Expand deterministic separating circuits across measurement bases, resets, pair products, MPP, feedback, herald records, repeats, observables, and frame selection.
- Run analytical distribution checks for supported noise gates, channels, correlated chains, measurement flips, heralded outcomes, and DEM mechanisms.
- Add two-sample Stim-versus-Stab plans only for cases without a tractable analytical expectation.
- Cross-check measurement sampling, reference sampling, detection conversion, ordinary detection sampling, detector-frame sampling, analyzer symptoms, and DEM sampling on shared circuits.
- Exercise all applicable output formats and streaming visitor paths.
- Add `full` and `soak` seed panels without allowing diagnostic reruns to erase failures.

### Tests

- Exact deterministic record and detector-event cases across all engines.
- Statistical bucket tests with shared integer boundaries and suite-wide budget checks.
- Cross-engine metamorphic tests comparing explicit error insertion with sampled channel effects and folded repeats with unrolled circuits.
- Replay and sampled-error equivalence for DEM sampling.
- Negative and resource cases for unsupported sweep shapes, invalid feedback references, excessive widths, visitor failure, writer failure, replay truncation, and materialized error-record caps.

### Acceptance Criteria

- Every supported probabilistic family has a non-vacuous effect test and a no-effect boundary test.
- The full statistical suite stays within its declared false-positive budget.
- Ordinary and detector-frame paths are independently selected and tested.
- Streaming paths retain bounded per-record or per-group buffers.

## Milestone CQ5: Analyzer, Search, Flows, Detector Utilities, And Transforms

### Objective

Qualify the complex semantic and algorithmic surfaces that are vulnerable to false completion through broad structural tests.

### Tasks

- Expand analyzer folded-versus-unrolled differential corpora across coordinates, gauges, observables, channels, decompositions, and generated codes.
- Compare graphlike and hypergraph searches against brute force on bounded models and against pinned Stim on selected larger models.
- Verify WCNF exact output where deterministic and structural satisfiability or optimum invariants where ordering differs.
- Expand flow generation, checking, solving, detecting-region, missing-detector, reverse-tracking, inverse-QEC, and feedback-inlining exact subcases.
- Test independent admission limits for traversal, graph construction, edge incidence, search state, clauses, literals, solver matrices, detector sets, qubit width, and repeat work.
- Keep full ErrorMatcher provenance and unselected transform shapes deferred.

### Tests

- Exact pinned analyzer, flow, detector-utility, inverse, and feedback cases with independent selectors.
- Deterministic property corpora for folded loops, generated circuits, GF(2) flow solving, small search models, and sparse high-id trackers.
- Comparator mutation tests for tie-sensitive search, wrong source mechanism, wrong target signature, missing flow sign, and reordered contractual output.
- Boundary tests at and beyond each independent work or memory limit.

### Acceptance Criteria

- No complex algorithm is closed by one broad fixture or one file-level selector.
- Every structural comparator proves the invariants it intentionally ignores and rejects defects in contractual fields.
- Search and solver limits reject before uncontrolled allocation or traversal.
- Deferred provenance and product shapes remain explicit.

## Milestone CQ6: Full, Soak, Audit, And Release Closure

### Objective

Run the complete suite, remove coverage loopholes, and publish durable qualification evidence.

### Tasks

- Add PR, nightly full, scheduled soak, and release-candidate workflows.
- Run the full suite from a clean committed revision on at least Linux x86-64 and Linux AArch64; add macOS coverage for deterministic Rust and CLI cases where supported.
- Record and triage every flaky, timing-sensitive, platform-specific, ignored, or unexpectedly skipped case.
- Ban ignored qualification cases unless they are explicitly `soak` and have an owner and reason.
- Run milestone-audit against every CQ milestone and full-code-review across test, oracle, statistical, resource, and documentation changes.
- Update the feature checklist, test-porting plan, completion report, and benchmark correctness prerequisites.

### Required Commands

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
just qualification::correctness-check
just qualification::correctness-run --tier pr
just qualification::correctness-run --tier full
just oracle::blockers --check-selectors
just oracle::run --implemented-only
just maintenance::pre-commit
```

The `soak` command is required in scheduled evidence but is not a local pre-commit gate.

### Acceptance Criteria

- Every selected case is implemented or evidence-closed, and no selected case is deferred, missing, stale, shared-primary, skipped, or ambiguous.
- Every upstream case in selected source files has a disposition.
- Every comparator and resource claim has adversarial validation.
- Full reports identify clean Stab and pinned Stim commits.
- Milestone-audit and full-code-review have no unresolved confirmed finding.
- Documentation agrees on selected scope, explicit deferrals, counts, commands, and report paths.

## Defect And Under-Specification Policy

- A failing qualification case is a product defect until disproved; do not weaken the comparator, seed plan, expected output, cap, or timeout merely to make it pass.
- Fix defects in already implemented selected surfaces within the owning CQ milestone.
- Do not implement a deferred product as an incidental test fix.
- If implementation reveals a real specification gap, log it in `docs/plans/milestone-spec-gaps.md` with exact subcases and continue other unblocked cases.
- A new deferral requires an explicit plan decision and checklist update; it cannot be introduced only in a test comment or progress report.

## Completion Deliverable

Create `docs/plans/comprehensive-correctness-qualification-report.md` containing the final inventory counts, comparator counts, domain completion table, statistical budget, resource evidence, platform matrix, commands, clean commit metadata, milestone-audit result, full-code-review result, and explicit deferred products.
