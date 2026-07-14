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

Schema version 3 normalizes the inventory into four arrays so source discovery is not confused with executable evidence, gives every evidence owner an explicit execution contract, and gives every property case a typed property-plan reference:

- `features`: the sixteen selected `CQ-*` domains and their exact `PERF-*` dependencies.
- `upstream_cases`: every extracted C++ GTest expansion and Python pytest semantic record with path, line, complete symbol, parameter subcase, parameterization kind, disposition, named deferred product where applicable, one or more domain-relevance ids, and zero or more executable ownership records carrying comparator and evidence owner.
- `public_api_items`: every default-feature reachable behavioral Rust API path, including re-exports, variants, fields, inherent methods, trait methods, and distinct generic trait implementations, with source span and exact evidence owner.
- `evidence_cases`: executable or planned ownership records that carry the comparator, primary and supporting selectors, resource contract, negative axes, performance groups, and status.

Every evidence case must include:

- `id`: stable qualification case id.
- `feature_id`: stable feature id from the domain matrix in this plan.
- `behavioral_surface`: Rust API, CLI, file format, engine, or resource boundary under test.
- `provenance`: upstream semantic case, public Rust API, oracle fixture, Rust regression, blocker-ledger evidence source, or a source-owned planned qualification case.
- `comparator`: `exact-bytes`, `exact-value`, `canonical`, `error-class`, `structural`, `state-equivalence`, `semantic-invariant`, `statistical`, `property`, or `resource`.
- `primary_selector`: one exact Cargo test, oracle fixture id, property target, or ops check that selects only this case.
- `supporting_selectors`: optional shared evidence that cannot replace the primary selector.
- `statistical_plan`: required only for statistical cases; implemented and evidence-close cases reference an existing source-owned oracle, blocker-ledger, or qualification plan, while planned cases reference their planned qualification-case owner until CQ1 supplies executable plan data.
- `property_plan`: required only for property cases; planned cases reference their future qualification-case owner, while implemented cases bind the generator domain, case count, seed panel or static corpus, maximum generated bytes, corpus digest where applicable, persistence policy, and subprocess execution mode.
- `resource_contract`: `streaming`, `bounded-materialized`, `bounded-search`, `constant-scratch`, or `not-applicable`, plus explicit limits or slope rules.
- `negative_axes`: named malformed, unsupported, overflow, path, width, count, or writer-failure cases.
- `performance_groups`: benchmark qualification groups whose timed workloads depend on this case.
- `deferred_product`: required only when `status=deferred`; this binds a deferred evidence case to one named deferred product instead of inferring product ownership from unrelated upstream rows in the same domain.
- `status`: `planned`, `implemented`, `evidence-close`, or `deferred`.
- `execution`: ordered tier membership, timeout, stdout cap, stderr cap, artifact cap, and expected-skip policy.

The manifest schema must deny unknown fields, enforce bounded row and string counts before expensive work, reject unsafe paths and symlinks, validate exact upstream anchors inside pinned files, and include a frozen semantic digest.
The checked manifest must exactly match deterministic regeneration from the pinned Stim tree, the default-feature rustdoc JSON graph, and current implemented oracle rows.
Case ids, API paths, and repository-relative source paths must deserialize through validated domain types, and public API classification must reject unknown ownership instead of using `CQ-RESOURCE` as a miscellaneous fallback.
The manifest header must record the exact pinned Python version used for isolated AST extraction, and regeneration must invoke that version explicitly so AST-derived case ids do not vary with the developer environment.
Static `pytest.mark.parametrize` values must expand through isolated AST into deterministic subcase records, including stacked-decorator Cartesian products; a dynamic expression must become one content-addressed `dynamic-family` record and cannot enter executable scope until a later milestone replaces it with finite explicit subcases.
Domain relevance and executable evidence ownership are separate: deferred and not-applicable cases remain visible in applicable domain summaries but own no passing evidence, while selected executable cases carry exact ownership records.
An atomic semantic case proves only its declared comparator; it must not inherit feature-wide negative axes or resource claims, which require dedicated cases that directly exercise those boundaries.
An existing Cargo primary selector must name exactly one concrete libtest case and use `--exact`; a broad Cargo filter may appear only as supporting evidence and cannot make a planned primary case pass.
Planned evidence records reserve their future `full` and `soak` tier ownership but are not executable and do not enter a passing run until their primary selector becomes exact and existing.
An implemented property target must be registered and execute in a killable qualification worker subprocess; an implemented static property corpus may use one exact Cargo selector only when its source path and content digest are frozen in the property plan.

### Operational Surface

Extend `stab-oracle` instead of adding shell scripts.
Expose thin recipes from a new modular `justfiles/qualification.just` file:

```sh
just qualification::correctness-list
just qualification::correctness-check
just qualification::correctness-regenerate --check
just qualification::correctness-run --tier pr
just qualification::correctness-run --tier full
just qualification::correctness-run --tier soak
just qualification::correctness-report --out target/qualification/correctness/latest
just qualification::correctness-preflight --out target/qualification/correctness/latest --case <qualification-case-id> --request-sha256 <run-request-sha256> --completion-sha256 <run-completion-sha256>
```

Complex selection, source-anchor validation, subprocess execution, timeout handling, statistical evaluation, and report generation belong in Rust under `ops/oracle`.
`correctness-run` validates the checked inventory and Stim source before the private builds, seals immutable copies of every direct executable, constructs private config-free homes and a hashed child environment, publishes `request.json` before executing the selected exact cases, records one canonical execution receipt per case, content-binds every bounded failure artifact by path, byte count, and SHA-256 digest, publishes `completion.json` over the canonical report and all case receipts, and atomically exchanges the complete run directory into its final location.
`correctness-report` consumes the canonical request, report, completion, and execution receipts, reconstructs the complete tier and filter selection from the checked manifest, validates exact case ownership, selectors, statistical plans, resource contracts, process status, output framing, artifacts, and current revision metadata, and deterministically regenerates only the derived Markdown and preflight artifacts without rerunning cases.
`correctness-preflight` consumes `preflight.json` and requires controller-approved request and completion digests so intended selection and completed outcomes are independently anchored; it rejects stale manifest, Stab, Stim, selector, output, selection, or case-result bindings, while dirty evidence requires an explicit `--allow-dirty` diagnostic opt-in and is not promotable completion evidence.
`--allow-deferred` is valid only with explicit `--case` filters, retains the selected deferred cases as visible diagnostic counts without executing them, and always produces a preflight that rejects promotion because its deferred count is nonzero.
Broad tier or feature selection never uses `--allow-deferred` to hide deferred scope, and an explicit planned or out-of-tier case remains an error.
CQ1 qualification execution is supported only on Linux because promotable runs require both killable process-group timeouts and atomic directory exchange; the runner must fail before executing a case on unsupported hosts instead of falling back to immediate-child termination or non-atomic publication.

### Report Contract

The generated JSON and Markdown reports must record:

- Stab commit and `local_modifications`.
- Stim tag and commit.
- Rust toolchain, target triple, operating system, and architecture.
- Selected tier, feature filters, seeds, shot totals, and property corpus ids.
- Planned statistical seeds and shots, exact completed comparisons, batches, and shots, unknown work when an exact completion record is unavailable, and every per-seed attempt outcome in execution order.
- Evidence pass, fail, planned, and deferred counts by domain and comparator.
- Upstream semantic-mining, not-applicable, deferred-product, exact-oracle, ported-Rust, and superseded disposition counts by domain, without inventing comparators for non-executable upstream rows.
- Every failed case with its exact primary selector and artifact path, byte count, and SHA-256 digest.
- Every deferred case with its named deferred product.
- Every executed case's exact selector digest and bounded output digest.
- Selection-completeness counts, planned ownership counts, property corpus ids, and exercised resource contracts.
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
- Report both the sum of declared per-case bounds and the exact consumed bound for independently completed comparisons, derived from the canonical integer rejection regions; shot batches must divide evenly across comparisons, each fixed Cargo marker must encode exactly that frozen per-comparison batch count after structural completion and before probabilistic acceptance, a malformed marker receives no credit while an earlier validated marker prefix remains credited, a failed attempt may retain an exact completed Stim or Stab side, and a passing attempt must prove every frozen comparison and shot batch.
- A failed statistical seed is a failure; diagnostic reruns may add evidence but may not replace the failed result.
- The `full` tier uses the frozen primary seed set, while the `soak` tier adds a deterministic seed panel and reports every seed.
- Source-owned statistical cases whose legacy Cargo selector does not expose a seed override retain their one exact source seed in soak; independently seedable oracle plans add the deterministic soak panel without replacing the primary seed.

### Property And Metamorphic

- Every generated property case must use a source-owned generator domain, maximum size, deterministic seed, and case count.
- An existing static property corpus is exempt from generated seeds and shrinking only when its source path, content digest, exact selector, and `existing-focused-regression` persistence policy are frozen in the manifest.
- Generated property failures must reproduce, shrink deterministically, use a target-bound bounded persistence format, and replay through the registered killable worker before the parent records the regression artifact.
- Persist minimized regressions as ordinary focused tests or committed corpus inputs after review.
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
| `CQ-BIT-KERNELS` | Selected Rust scalar and portable-SIMD dense bits, bit tables, transposition, Boolean and popcount helpers, and sparse XOR helpers used by selected engines | Scalar differential checks, unaligned tails, empty and large widths, safe-Rust overlap policy, sparse-density crossover, allocation and mutation boundaries | Portable semantic subsets of SIMD bit, bit-table, word, sparse-XOR, and integer-twiddle GTests | `PERF-BIT-KERNELS` |
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

`CQ-BIT-KERNELS` does not create public Rust operations merely to mirror C++ internal storage mechanics. C++ moved-from state, mutable aliasing, destructive or preserving resize, padded lane layout, unexposed arithmetic or shifts, raw bit-vector random fill, table concatenation, and other helpers with no selected Stab API or engine contract receive exact `not-applicable` dispositions. Typed randomization exposed by Pauli, Clifford, or Tableau APIs is qualified under `CQ-ALGEBRA` with caller-owned RNG contracts.

## Milestone CQ0: Freeze The Case-Level Inventory

### Objective

Convert the current file-level test hierarchy and feature checklist into a finite case-level qualification inventory before adding new tests.

### Tasks

- Parse all GTest macro declarations from the 103 pinned C++ test files and all pytest function declarations from the 91 pinned Python test files.
- Generate a deterministic rustdoc JSON inventory for selected `stab-core` and `stab-cli` exports and assign every public item to a qualification feature, explicit parent contract, or documented non-semantic API disposition.
- Define the public API filter as default-feature reachable types, functions, constants, inherent methods, and explicitly implemented public traits; exclude compiler-generated auto or blanket implementation noise and the evidence-only `ops-contracts` feature from product API counts while testing that evidence-only exports do not leak into default builds.
- Record the exact extraction grammar, including `TEST`, `TEST_F`, `TYPED_TEST`, and Stim word-size macros; reject ambiguous or duplicate anchors.
- Use Python's isolated standard-library AST through `uv` to find module, class, and async pytest functions without executing test modules or accepting declarations hidden in comments, strings, or nested helper functions.
- Expand statically enumerable `pytest.mark.parametrize` decorators, including stacked Cartesian products, into deterministic subcases; record dynamic expressions as one content-addressed non-executable family instead of silently collapsing them into one executable test.
- Expand Stim's word-size macro into explicit 64-bit, 128-bit, and 256-bit semantic subcases so architecture-dependent C++ preprocessing cannot erase portable-SIMD ownership.
- Classify every upstream case in a file that contributes to an implemented selected surface.
- Import existing blocker-ledger, oracle, and Cargo-test evidence by stable id without copying its content.
- Assign every upstream record one or more domain-relevance ids where applicable and assign every selected executable record one or more exact primary qualification owners without allowing deferred records to contribute passing evidence.
- Name all deferred-product and not-applicable cases explicitly.
- Freeze independent planned `CQ-RESOURCE` owners for parser admission, checked count arithmetic, result-record admission, materialized expansion, streaming buffer slope, writer failure, visitor failure, replay and side-input admission, folded traversal work, search and solver admission, allocation scaling, typed path boundaries, and output-file lifecycle; retain symlink rejection as its own implemented owner.
- Add a semantic digest and bounded validation before source-file reads or statistical work.

### Tests

- Parser tests for every supported GTest macro form and pytest function form.
- Regressions for declaration prefixes, comments, strings containing fake declarations, duplicate names, one-character gates, longer gate-name substrings, and anchors present only in declarations.
- Manifest tests for duplicate ids, missing fields, unsafe paths, symlinks, unknown dispositions, stale evidence ids, shared primary selectors, unknown feature ids, and digest mismatch.
- Manifest tests for bounded strings and arrays, bounded diagnostics, invalid digest shape, missing statistical-plan owners, invalid behavioral-surface or provenance combinations, and semantic cases that overclaim negative or resource evidence.
- Python extractor tests for static, stacked, and dynamic parameterization, with validation that dynamic parameter families cannot enter executable scope.
- Public API inventory tests for duplicate canonical paths, stale items, undocumented additions, re-exports, trait implementations, feature-gated exports, and items mapped to missing cases.
- A completeness test proving every selected feature id has an executable or explicitly evidence-closed case and every deferred case names its product.
- A resource-inventory test proving every required boundary family has its own source-owned primary case and cannot disappear behind the existing symlink regression.

### Acceptance Criteria

- `just qualification::correctness-check` reports exact counts by disposition and domain.
- No selected feature is represented only by a file-level row.
- No selected exported Rust API item is unclassified or represented only by module-level evidence.
- No implemented case has a shared or non-resolving primary selector.
- `CQ-RESOURCE` contains the exact finite boundary-family inventory required by this milestone, with one independent planned or implemented owner per family.
- The inventory can be regenerated deterministically from pinned Stim and checked without modifying it.
- `just qualification::correctness-regenerate --check` byte-compares the checked manifest with fresh pinned-source and rustdoc discovery.

## Milestone CQ1: Build The Qualification Harness

Status: Complete as of 2026-07-14. Clean PR, full, soak, report, preflight, audit, and review evidence is recorded in [cq1-correctness-harness-progress-report.md](cq1-correctness-harness-progress-report.md).

### Objective

Make every comparator, selector, statistical plan, artifact, timeout, and report contract executable before expanding feature tests.

### Tasks

- Add tier and feature filtering to `stab-oracle` qualification commands.
- Reuse existing exact fixture and Cargo-test execution paths.
- Add canonical, state-equivalence, semantic-invariant, property, and resource result adapters with typed outputs.
- Add suite-wide statistical-budget validation and deterministic seed-panel expansion.
- Make each statistical plan declare independent comparisons and shot batches per attempt, reject plans whose batches do not divide evenly across comparisons, require each fixed Cargo marker to match the derived batches per comparison, charge the exact union-bound multiplicity, credit each compared side only from an exact structurally valid record count, retain validated marker prefixes on malformed suffixes, and require all declared sides and batches before an attempt can pass.
- Add typed property execution plans with deterministic generation, shrinking, bounded persisted regressions, static-corpus digests, and killable worker execution.
- Add per-case subprocess timeouts, prompt bounded stdout and stderr enforcement, process-group cleanup after timeout, output overflow, controller cancellation, and normal direct-child exit, plus safe artifact paths under `target/qualification/`.
- Validate the actual Stim checkout commit, exact tag, tracked state, and untracked state before execution and build fresh private Stab and Stim binaries for each qualification run instead of reusing mutable shared build outputs.
- Resolve the Cargo, Rust, C/C++, CMake, Make, Git, worker, Stab, and Stim executables to source-owned direct roles, copy their bytes into sealed Linux memory files, hash and size-bind them in canonical order, keep the sealed descriptors through execution, allocate private runtime state under fixed `/tmp`, invoke Cargo from `/` with absolute manifest paths, inspect repository metadata through a private config-free Git view whose index is reconstructed from `HEAD`, snapshot compiler and CMake support trees into read-only content-bound directories, seal compiler subordinate programs, revalidate support snapshots after every compiler-consuming case and before publication, remove private scratch through retained descriptors after the run, and run qualification children under a hashed explicit environment allowlist that binds the support-tree digests.
- Give fixture-producing children inherited descriptor-relative side-output paths, monitor and read those outputs with no-follow descriptor-relative opens, and clean their private output directories with the same bounded identity-checked traversal used for qualification artifacts.
- Add canonical pre-execution request receipts, per-case execution receipts, and post-execution completion receipts so report outcomes, executable identities, and exact selected work cannot be rewritten independently.
- Write staged artifacts relative to the staging directory descriptor, synchronize every newly created nested directory with its parent, anchor publication locks and output-parent traversal to a retained repository descriptor, identity-check the complete parent chain before and after atomic publication or report regeneration, and perform bounded iterative descriptor cleanup only after durable publication and lock release.
- Make report generation fail when a selected case is missing, deferred without selection permission, stale, unowned, or unexpectedly skipped.
- Add correctness preflight ids that the performance suite can require before timing a workload.
- Promote only exact single-case Cargo selectors to executable primary evidence; retain inherited broad filters only as supporting evidence behind planned atomic qualification owners.

### Tests

- Comparator mutation tests that introduce one wrong byte, target, sign, weight, count, tag, coordinate, exit status, or error class.
- Statistical tests for integer boundaries, impossible plans, excess suite budget, missing buckets, duplicate seeds, independent-comparison and shot-batch multiplicity, uneven or incorrect per-comparison batch shapes, missing, malformed, stale, and out-of-order completion markers, validated-prefix retention before a malformed suffix, partial completed-side credit on failed attempts, completion emitted before probabilistic rejection, complete rejected distributions receiving exact credit, rerun-until-pass rejection, both Stim and Stab comparison tails, and budgeted multi-seed soak expansion.
- Property-runner tests for deterministic seeds, shrinking, persisted regressions larger than the subprocess stdout cap, killable replay and timeout workers, and oversized generated cases.
- Report tests for dirty worktrees, wrong Stim commit, missing selectors, self-consistent partial or altered selections, rewritten outcomes, stale request or completion digests, mismatched executable ledgers, incomplete or oversized stream receipts, wrong exact Cargo test counts, stale artifact bytes, artifact traversal attempts, and non-UTF-8 subprocess output.
- Selection tests for explicit planned, deferred, feature-mismatched, and out-of-tier cases, including the diagnostic-only `--allow-deferred` contract.
- Process tests proving output overflow terminates promptly, sticky controller cancellation kills the process group and prevents later children or publication including cancellation received while waiting for the publication lock, normal parent exit kills closed-stdio descendants, and qualification children inherit only the explicit environment.
- Executable tests proving sealed bytes continue to run after path replacement and in-place source mutation, external Cargo invocation runs from `/` and ignores manifest or scratch ancestor configuration, the private Git view ignores repository-local configuration and caller index flags, private Cargo homes link only caches and not caller configuration or credentials, support snapshots reject content changes and empty the owned read-only tree without deleting a replacement root, descriptor-owned fixture side outputs survive real Stim and Stab CLI execution, and `just qualification::correctness-provenance-probe` runs one real source-owned case before validating its published request, execution, report, completion, and preflight bindings.
- Publication tests proving an abandoned staging directory is removed, a complete rerun removes stale prior artifacts, concurrent publications serialize without orphaned staging directories, staged writes remain descriptor-owned after a path swap, publication and report regeneration reject a replaced output-parent chain, nested directory creation is synchronized, cleanup depth is bounded without invalidating a durable new publication, over-budget private runtimes are quarantined without an unbounded fallback, symlink swaps cannot redirect cleanup or publication, and unsupported hosts fail closed before qualification execution.

### Acceptance Criteria

- All comparator classes have adversarial unit tests.
- A deliberately weakened or missing case makes the suite fail.
- Reports are reproducible, bounded, identify exact commits, selectors, and executable identities, reconstruct their complete selected case set from the frozen manifest, derive each outcome from a canonical execution receipt, and content-bind every retained artifact.
- Qualification execution uses a validated Stim checkout, fresh private Stab and Stim builds, sealed direct and compiler-subordinate executables, Cargo invoked from `/` with private configuration, config-free Git metadata inspection with an index reconstructed from `HEAD`, read-only content-bound compiler and CMake support snapshots revalidated after compiler use and before publication, bounded descriptor-owned private-runtime and side-output cleanup, a hashed explicit child environment, exact statistical completion receipts, prompt sticky process-group cleanup, a post-lock cancellation check, repository-anchored crash-durable descriptor-owned artifacts, and identity-checked locked report regeneration.
- The progress report names the controlled-host trust root explicitly, including the outer bootstrap, kernel, procfs and dynamic-loader behavior, system shared libraries, dependency caches, the live-checkout no-concurrent-mutation requirement, and the distinction between recorded hashes and authenticated tool provenance.
- Performance workloads can name and validate correctness prerequisites without parsing Markdown.
- PR, full, and soak runs complete their selected executable cases without an unexpected skip, and a generated preflight validates exact manifest, commit, selector, output, and result bindings.

## Milestone CQ2: Deterministic Models, Formats, Generation, And Algebra

### Objective

Complete deterministic and property coverage for `CQ-STIM-FORMAT`, `CQ-DEM-FORMAT`, `CQ-RESULT-FORMATS`, `CQ-GATE-CONTRACT`, `CQ-BIT-KERNELS`, `CQ-CIRCUIT-API`, `CQ-GENERATION`, and `CQ-ALGEBRA`.

### Tasks

- Maintain schema-version-2 `oracle/qualification-cases.json` as the source-owned exact-parent ledger. A focused qualification entry must bind complete upstream anchors and exported-API owners to one independently selectable case in the same feature. An existing-parent mapping may instead reuse one canonical implemented or evidence-close blocker-ledger, oracle-fixture, or Rust-regression parent in the same feature; the reviewed exact parent owns the final comparator because the discovered feature-level comparator is provisional. Validation must reject stale owners, duplicate claims, cross-feature claims, unsupported existing-parent provenance, broad Cargo filters, reused primary selectors, and mappings to planned parents.
- Expand word-size families only from explicit source-owned 64-bit, 128-bit, and 256-bit member lists. Validation must reject empty, duplicate, unsupported, missing, or stale members instead of accepting symbol patterns or file-level ownership.
- Review parent mappings semantically instead of collapsing owners by file, module, or name similarity. Add or split focused tests whenever an existing selector does not exercise every mapped constructor, accessor, accepted value, rejection boundary, canonical output, or property named by the owners.
- When one exact upstream symbol mixes selected portable behavior with incompatible language-specific or API-specific behavior and the pinned source has no independently addressable subcase, disposition the complete symbol according to its complete contract. Own shared portable semantics through an independent exact Rust parent instead of claiming partial-symbol parity or silently dropping the aggregate.
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
- Every selected exported Rust API item has a direct exact case or a reviewed exact-parent mapping, and deterministic regeneration proves that no planned owner disappeared without such a mapping.
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
