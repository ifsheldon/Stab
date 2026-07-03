# Goal: Finish The Partial Feature Closure Plan

## Purpose

This document is the active execution contract for finishing `docs/plans/partial-feature-closure-plan.md`.
The goal is to close every `Partial` feature-checklist row whose remaining work is not intentionally deferred, while keeping deferred Stim surfaces explicitly out of scope.
Completion means Stab has tested, benchmarked, documented, audited, and reviewed Rust and CLI evidence for the active partial surfaces named in the plan.

Use `docs/plans/lessons-learned.md` throughout this work.
The most important lesson is that exact subcases, executable comparators, benchmark classes, resource limits, and documented deferrals must be established before implementation starts.

## Active Scope

Included:

- Classify every `Partial` row in `docs/stab-feature-checklist.md` as a rollup, an active implementation row, or an explicit deferral.
- Finish non-deferred Rust core API gaps for circuit mutation, introspection, coordinates, reference samples, determined measurements, gate metadata, and basic DEM ergonomics.
- Finish active circuit transform gaps for flattening, noise removal, decomposition, feedback inlining, time reversal for flows, and repeat traversal where they are selected by the plan.
- Finish active sweep-conditioned execution and gate-semantic gaps for sampler, converter, detection, and analyzer paths.
- Finish active DEM API, transform, coordinate, introspection, folded traversal, and analysis gaps.
- Finish active detector utility and flow gaps for detecting regions, missing detectors, feedback-related transforms, and measurement-rich flows.
- Finish active analyzer, search, and sparse reverse tracking gaps without taking on deferred `explain_errors` CLI or full provenance work.
- Finish visible CLI parity gaps for `stab m2d`, `stab analyze_errors`, and accepted legacy aliases.
- Add or update source-owned tests, oracle rows, benchmarks, profiler notes, reports, checklist entries, roadmap text, and completion evidence in the same change set as behavior changes.
- Run milestone-audit and full-code-review before claiming completion.

Excluded:

- Do not implement Python bindings or Python-style API clone behavior as part of this goal.
- Do not implement JavaScript/WASM, diagrams, `stim explain_errors` CLI, `stim repl`, QASM, Quirk, Crumble, GPU, ecosystem packages, exact random-stream parity, C++ header compatibility, or new public graph/vector simulator APIs.
- Do not implement or document deprecated `--detector_hypergraph` as a supported Stab alias.
- Do not mark public `TableauSimulator` or `FlipSimulator` APIs complete; current simulator internals remain implementation support unless a later plan changes scope.
- Do not promote report-only, proxy, partial-match, tiny, or no-ratio benchmark rows into the 1.25x primary threshold gate without a separate source-owned rationale and stable repeated evidence.

## Sources Of Truth

- Active plan: `docs/plans/partial-feature-closure-plan.md`.
- PF0 inventory: `docs/plans/partial-feature-inventory.md`.
- Planning lessons: `docs/plans/lessons-learned.md`.
- Feature status: `docs/stab-feature-checklist.md`.
- Stim inventory: `docs/stim-feature-list.md`.
- Roadmap and milestone policy: `docs/plans/rust-stim-drop-in-rewrite.md`.
- Test hierarchy: `docs/plans/stim-test-porting-plan.md`.
- Spec-gap log: `docs/plans/milestone-spec-gaps.md`.
- Upstream baseline: pinned Stim v1.16.0 under `vendor/stim`.
- Oracle manifest: `oracle/fixtures/manifest.csv`.
- Benchmark manifest, thresholds, waivers, and profiler notes: `benchmarks/manifest.csv`, `benchmarks/m12-primary-thresholds.json`, `benchmarks/m12-primary-beta-waivers.json`, and `benchmarks/profiler-notes/`.

If these sources disagree about scope, status, benchmark class, command behavior, report paths, or deferrals, fix the stale source before claiming progress.

## Success State

The goal is complete only when:

- Every active partial row in `docs/stab-feature-checklist.md` is either implemented with evidence or remains explicitly partial with a named deferred subcase.
- Every rollup partial row has child-row evidence and does not imply broader parity than the implemented surfaces prove.
- Every implemented feature has tests that were added or ported before or alongside implementation.
- Every public CLI behavior is covered by CLI tests or oracle rows that prove stdout behavior, stderr class, exit status, accepted flags, rejected flags, path handling, input formats, output formats, and resource behavior.
- Every public Rust API added by this plan uses typed domain boundaries and clear domain errors.
- Every benchmarked feature has a source-owned manifest row, runner coverage, measurement work, compare notes, and a comparability class.
- Every row promoted into the 1.25x primary threshold gate has repeated stable direct or CLI-comparable evidence and matching profiler notes.
- Report-only rows are labeled as report-only and are not used as release-gate evidence.
- `docs/stab-feature-checklist.md`, `docs/plans/rust-stim-drop-in-rewrite.md`, oracle metadata, benchmark metadata, and milestone reports agree with current behavior.
- Milestone-audit and full-code-review findings are fixed, or under-specification findings are logged in `docs/plans/milestone-spec-gaps.md`.

## Execution Rules

- Start each milestone by extracting exact owned subcases from upstream tests, docs, or current Stab gaps.
- Add or port targeted tests before implementing the behavior they prove.
- Do not treat an upstream file as an acceptance criterion; split it into owned, semantic-mining, deferred, and out-of-scope subcases.
- Keep unsupported behavior explicit through clear rejections, manifest-only future rows, or documented deferrals.
- Keep public parser, converter, sampler, analyzer, and writer paths streaming where practical; otherwise document and test a resource cap.
- Use typed identifiers, paths, result formats, probabilities, coordinates, detector ids, observable ids, qubit ids, measurement references, repeat counts, and options after external boundaries.
- Prefer existing Stab parser, circuit, DEM, stabilizer, sampler, analyzer, oracle, and benchmark infrastructure over parallel implementations.
- Do not commit unless the user explicitly asks for commits.
- If final completion is claimed after a requested commit, regenerate final evidence from committed code with `local_modifications=false`.

## Milestone Work Loop

For every milestone in `docs/plans/partial-feature-closure-plan.md`:

1. Confirm the milestone scope against the feature checklist, Stim inventory, roadmap, lessons learned, and spec-gap log.
2. Split upstream references into exact owned subcases, semantic-mining references, explicit deferrals, and out-of-scope items.
3. Add or port targeted tests first, including negative tests for unsupported shapes and resource-boundary tests for public inputs and outputs.
4. Implement the feature with the narrowest code changes that fit existing Stab patterns.
5. Run the milestone’s targeted tests and fix failures.
6. Add or update benchmark rows, benchmark runner coverage, measurement work units, compare notes, profiler notes, and threshold entries when required.
7. Update docs, oracle metadata, benchmark metadata, feature checklist status, roadmap text, and progress or completion reports in the same change set as behavior changes.
8. Check the milestone acceptance criteria from `docs/plans/partial-feature-closure-plan.md`.
9. Run milestone-audit and fix implementation, evidence, or documentation issues.
10. Run full-code-review for the touched surfaces and fix findings.
11. If audit or review exposes under-specified scope, log it in `docs/plans/milestone-spec-gaps.md` and keep the corresponding checklist entry partial or deferred.

## Milestone Evidence

PF0 evidence:

- Every active partial row has an owner milestone, comparator class, planned tests, planned benchmarks if relevant, and explicit exclusions.
- `docs/plans/partial-feature-inventory.md` maps active partial rows, rollup rows, deferred-only rows, oracle placeholders, benchmark placeholders, tests, and exclusions.
- `just oracle::list`, `just oracle::matrix --check`, `cargo test -p stab-oracle fixtures --quiet`, and `just bench::list` pass or report only documented future rows.

PF1 evidence:

- Core Rust circuit, gate, reference-sample, determined-measurement, coordinate, and basic DEM API gaps selected by the plan have behavior tests and typed API documentation.
- Report-only or comparable benchmark rows exist for high-volume introspection, coordinate, and metadata paths.

PF2 evidence:

- Circuit transform APIs have exact or semantic tests for flattening, noise removal, decomposition, feedback inlining, time reversal for flows, and repeat traversal.
- Transform benchmarks exist with paired submeasurements for mixed workloads.

PF3 evidence:

- Sweep-conditioned and gate-semantic execution gaps have core and CLI tests across selected result formats and unsupported-shape errors.
- Sweep and gate execution benchmark rows are classified and smoke-tested.

PF4 evidence:

- DEM introspection, transform, coordinate, folded traversal, and analysis APIs have exact, structural, negative, and resource-boundary tests.
- DEM transform and folded traversal benchmarks exist and are not promoted to gates without stable comparable evidence.

PF5 evidence:

- Detector utility and flow APIs have tests for every promoted detecting-region, missing-detector, feedback, and measurement-rich flow subcase.
- Utility benchmarks are report-only unless a faithful pinned Stim baseline and stable repeated evidence justify promotion.

PF6 evidence:

- Analyzer, search, and sparse reverse tracker gaps have exact or structural parity tests, generated-circuit coverage, loop-folding evidence, and fuzz or generated tests where useful.
- Analyzer and search benchmarks use submeasurement thresholds when bundled rows could hide slow subcases.

PF7 evidence:

- `stab m2d`, `stab analyze_errors`, and accepted legacy aliases have CLI tests and oracle rows for accepted behavior, rejected behavior, path errors, resource limits, and excluded aliases.
- `--detector_hypergraph` remains consistently excluded from CLI parity.

PF8 evidence:

- Fresh benchmark reports exist for newly gated rows and record metadata, warmup, repeated measurements, local-modification state, and profiler notes.
- Feature checklist, roadmap, oracle manifest, benchmark manifest, threshold files, waivers, profiler notes, and completion reports agree.

## Targeted Test Commands

Use focused checks during implementation, expanding filters as touched code grows:

```sh
cargo test -p stab-core circuit --quiet
cargo test -p stab-core dem --quiet
cargo test -p stab-core flow --quiet
cargo test -p stab-core detection --quiet
cargo test -p stab-core analyze --quiet
cargo test -p stab-cli m2d --quiet
cargo test -p stab-cli analyze_errors --quiet
cargo test -p stab-oracle fixtures --quiet
cargo test -p stab-bench --quiet
just oracle::run --implemented-only
just bench::smoke
```

Add narrower filters when a change touches a specific transform, sweep branch, gate execution branch, DEM traversal, oracle comparator, benchmark validator, or CLI parser branch.
Avoid tests that only restate constants or static labels.

## Benchmark Evidence Rules

Before adding or promoting benchmark rows:

- Classify the row as `direct-match`, `cli-baseline`, `contract-representative`, `contract-proxy`, `contract-smoke`, `partial-match`, `report-only`, or `contract-only`.
- Define measurement work units before collecting evidence.
- Add compare notes explaining whether the pinned Stim baseline is faithful.
- Use warmup and repeated measurement runs for rows that could become gates.
- Use schema-version-2 submeasurement thresholds for bundled rows.
- Keep no-ratio rows out of the primary threshold file unless a source-owned waiver explains why that is acceptable.

For newly gated rows, run fresh evidence from current `HEAD`:

```sh
just bench::baseline --primary --out target/benchmarks/partial-feature-primary-baseline
just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --baseline target/benchmarks/partial-feature-primary-baseline/baseline.json --report target/benchmarks/partial-feature-primary-compare
just bench::primary-regression --baseline target/benchmarks/partial-feature-primary-baseline/baseline.json --report target/benchmarks/partial-feature-primary-regression
```

Do not cite exploratory probe reports as final release evidence.

## Required Final Verification

Before claiming the plan complete, run:

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings
cargo test --workspace --quiet
just oracle::run --implemented-only
just bench::smoke
just maintenance::pre-commit
```

If the plan changes benchmark gates, also run the benchmark evidence commands above.
If implementation changes shared parser, sampler, analyzer, stabilizer, oracle, benchmark, or CLI infrastructure, expand verification beyond the targeted filters.
If a required local tool or oracle binary is missing, install it through the documented project workflow or report the blocker.

## Stop And Log Conditions

Stop implementation work and write a spec-gap entry when:

- A promoted subcase requires Python bindings, JS/WASM, diagrams, `explain_errors` CLI, `repl`, QASM, Quirk, Crumble, GPU, ecosystem integrations, exact random-stream parity, or public Python-style simulator APIs.
- A promoted subcase still depends on a whole upstream file instead of exact owned subcases.
- A public CLI surface cannot define accepted flags, rejected flags, input formats, output formats, stdout behavior, stderr class, exit status, and resource behavior.
- A benchmark row cannot be classified or cannot produce faithful comparable evidence.
- A row would need stale reports, informal waivers, or unrecorded local modifications to pass.
- A public parser, converter, sampler, analyzer, or writer path has neither streaming behavior nor a documented cap.
- A checklist update would need to overstate implemented behavior to mark a row done.
