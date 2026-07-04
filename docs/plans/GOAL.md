# Goal: Finish Remaining Non-Deferred Partial Features

## Purpose

This document is the active execution contract for finishing `docs/plans/remaining-partial-feature-milestones.md`.
The goal is to close every `Partial` feature-checklist row whose remaining work is not intentionally deferred, while keeping deferred Stim surfaces explicitly out of scope.
Completion means Stab has tested, benchmarked, documented, audited, and reviewed Rust and CLI evidence for the active partial surfaces named in the plan.

Use `docs/plans/lessons-learned.md` throughout this work.
The most important lesson is that exact subcases, executable comparators, benchmark classes, resource limits, and documented deferrals must be established before implementation starts.

## Active Scope

Included:

- Finish the active milestones RPF0 through RPF8 in `docs/plans/remaining-partial-feature-milestones.md`.
- Classify every `Partial` row in `docs/stab-feature-checklist.md` as a rollup, an active implementation row, a mixed row with active and deferred parts, or a deferred-only row.
- Finish non-deferred Rust core API, transform, DEM, analyzer, search, flow, sweep-conditioned execution, gate metadata, and gate execution gaps.
- Finish non-deferred CLI gaps for `stab m2d`, `stab analyze_errors`, and accepted legacy aliases.
- Add or update source-owned tests, oracle rows, benchmarks, profiler notes, progress reports, completion reports, checklist entries, roadmap text, and spec-gap logs in the same change set as behavior changes.
- Run milestone-audit and full-code-review before claiming a milestone complete.

Excluded:

- Do not implement Python bindings or Python API clone behavior as part of this goal.
- Do not implement JavaScript/WASM, diagrams, `stim explain_errors` CLI, `stim repl`, QASM, Quirk, Crumble, GPU, ecosystem packages, exact random-stream parity, C++ header compatibility, or new public graph/vector simulator APIs.
- Do not implement public `TableauSimulator` or `FlipSimulator` products under this goal.
- Do not implement or document deprecated `--detector_hypergraph` as a supported Stab alias.
- Do not promote report-only, proxy, partial-match, tiny, or no-ratio benchmark rows into the 1.25x primary threshold gate without a source-owned rationale, stable repeated evidence, and matching profiler notes.

## Sources Of Truth

- Active plan: `docs/plans/remaining-partial-feature-milestones.md`.
- Historical PF plan: `docs/plans/partial-feature-closure-plan.md`.
- PF inventory: `docs/plans/partial-feature-inventory.md`.
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

## Execution Rules

- Start every milestone by extracting exact owned subcases from the checklist, Stim inventory, pinned upstream docs, pinned upstream tests, and current Stab gaps.
- Split upstream references into owned subcases, semantic-mining references, explicit deferrals, and out-of-scope items.
- Add or port targeted tests before implementing the behavior they prove.
- Do not treat an upstream file as an acceptance criterion.
- Keep unsupported behavior explicit through clear rejections, manifest-only future rows, or documented deferrals.
- Keep public parser, converter, sampler, analyzer, transformer, search, and writer paths streaming where practical; otherwise document and test a resource cap.
- Use typed identifiers, paths, result formats, probabilities, coordinates, detector ids, observable ids, qubit ids, measurement references, repeat counts, and options after external boundaries.
- Prefer existing Stab parser, circuit, DEM, stabilizer, sampler, analyzer, oracle, benchmark, and CLI infrastructure over parallel implementations.
- Commit only when the user explicitly asks for commits or an active goal explicitly authorizes commits; group commits logically.
- If final completion is claimed after a requested commit, regenerate final evidence from committed code with `local_modifications=false`.

## Milestone Work Loop

For every milestone in `docs/plans/remaining-partial-feature-milestones.md`:

1. Confirm the milestone scope against `docs/stab-feature-checklist.md`, `docs/stim-feature-list.md`, `docs/plans/lessons-learned.md`, and `docs/plans/milestone-spec-gaps.md`.
2. Extract exact owned upstream subcases and record deferred subcases before coding.
3. Add or port targeted tests first, including negative tests and resource-boundary tests for public inputs and outputs.
4. Implement the feature with the narrowest code changes that fit existing Stab patterns.
5. Run the milestone targeted tests and fix failures.
6. Add or update oracle rows, benchmark rows, benchmark runner coverage, measurement work units, compare notes, profiler notes, waivers, and threshold entries when required.
7. Update docs, feature checklist status, roadmap text, oracle metadata, benchmark metadata, and progress or completion reports in the same change set as behavior changes.
8. Check the milestone acceptance criteria from `docs/plans/remaining-partial-feature-milestones.md`.
9. Run milestone-audit and fix implementation, evidence, or documentation issues.
10. Run full-code-review for the touched surfaces and fix findings; when working in Codex, spawn GPT-5.5/xhigh subagents during the review.
11. If audit or review exposes under-specified scope, log it in `docs/plans/milestone-spec-gaps.md` and keep the corresponding checklist entry partial or deferred.

## Milestone Evidence

RPF0 evidence:

- Every partial row has a classification, owner milestone or deferral reason, upstream subcase list, comparator class, oracle status, benchmark status, and exclusion list.
- `docs/plans/partial-feature-inventory.md` agrees with `docs/plans/remaining-partial-feature-milestones.md`.
- `just oracle::list`, `just oracle::matrix --check`, `cargo test -p stab-oracle fixtures --quiet`, and `just bench::list` pass.

RPF1 evidence:

- Gate decomposition metadata, measurement-rich or variable-target flow metadata decisions, unsupported accessor behavior, and canonical gate execution support are tested and documented.
- `pf1-gate-metadata-lookup` or its replacement covers every implemented metadata accessor with measurement work and compare notes.

RPF2 evidence:

- Circuit `flattened`, `without_noise`, `decomposed`, feedback inlining, repeat traversal, and flow-time-reversal behavior have exact or semantic tests and resource-boundary tests.
- Transform benchmarks exist for performance-sensitive transforms and are classified before any threshold promotion.

RPF3 evidence:

- Sweep-conditioned execution and legal-gate execution gaps have core and CLI tests across selected result formats, accepted paths, rejected shapes, and streaming or capped resource behavior.
- Sweep and gate execution benchmarks are classified and smoke-tested.

RPF4 evidence:

- DEM `flattened`, `rounded`, tag stripping, coordinate/count queries, folded traversal, and transform resource boundaries have exact, structural, negative, and resource-boundary tests.
- DEM transform and folded traversal benchmarks exist and remain report-only unless faithful comparable evidence justifies promotion.

RPF5 evidence:

- Detecting regions, missing detectors, measurement-rich flows, flow validation, and flow-aware transforms have tests for every promoted subcase and precise errors for unpromoted families.
- Utility benchmarks are report-only unless faithful pinned Stim comparison and stable ratios justify promotion.

RPF6 evidence:

- Analyzer, search, sparse reverse tracking, and active matched-error value-object gaps have exact or structural parity tests, generated-circuit coverage, loop-folding evidence, and fuzz or generated tests where useful.
- Analyzer and search benchmarks use submeasurement thresholds when bundled rows could hide slow subcases.

RPF7 evidence:

- `stab m2d`, `stab analyze_errors`, and accepted legacy aliases have CLI tests and oracle rows for accepted behavior, rejected behavior, path errors, resource limits, output formats, side outputs, stdout behavior, stderr class, and exit status.
- `--detector_hypergraph` remains consistently excluded from CLI parity.

RPF8 evidence:

- Fresh benchmark reports exist for newly gated rows and record metadata, warmup, repeated measurements, local-modification state, and profiler notes.
- Feature checklist, roadmap, oracle manifest, benchmark manifest, threshold files, waivers, profiler notes, and completion reports agree.
- Milestone-audit and full-code-review findings are fixed, or under-specification findings are logged in `docs/plans/milestone-spec-gaps.md`.

## Targeted Test Commands

Use focused checks during implementation, expanding filters as touched code grows:

```sh
cargo test -p stab-core circuit --quiet
cargo test -p stab-core gate --quiet
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
just bench::baseline --primary --out target/benchmarks/remaining-partial-primary-baseline
just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --baseline target/benchmarks/remaining-partial-primary-baseline/baseline.json --report target/benchmarks/remaining-partial-primary-compare
just bench::primary-regression --baseline target/benchmarks/remaining-partial-primary-baseline/baseline.json --report target/benchmarks/remaining-partial-primary-regression
just bench::primary-memory-regression --baseline target/benchmarks/remaining-partial-primary-baseline/baseline.json
```

Do not cite exploratory probe reports as final release evidence.

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

- A promoted subcase requires Python bindings, JS/WASM, diagrams, `explain_errors` CLI, `repl`, QASM, Quirk, Crumble, GPU, ecosystem integrations, exact random-stream parity, C++ header compatibility, or a public simulator product.
- A promoted subcase still depends on a whole upstream file instead of exact owned subcases.
- A public CLI surface cannot define accepted flags, rejected flags, input formats, output formats, stdout behavior, stderr class, exit status, path handling, and resource behavior.
- A benchmark row cannot be classified or cannot produce faithful comparable evidence.
- A row would need stale reports, informal waivers, or unrecorded local modifications to pass.
- A public parser, converter, sampler, analyzer, transformer, search, or writer path has neither streaming behavior nor a documented cap.
- A checklist update would need to overstate implemented behavior to mark a row done.
