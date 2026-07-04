# Goal: Execute Remaining Partial Feature Milestones

## Mission

Finish the active milestones in `docs/plans/remaining-partial-feature-milestones.md`.
The objective is to close every `Partial` row in `docs/stab-feature-checklist.md` whose remaining work is not intentionally deferred.
Completion means the implemented Rust and CLI surfaces have source-owned tests, oracle evidence, benchmark evidence where relevant, synchronized documentation, milestone-audit closure, and full-code-review closure.

Use `docs/plans/lessons-learned.md` before planning or implementing each milestone.
The recurring failure to avoid is claiming progress from broad checklist wording, broad upstream test files, stale benchmark reports, or nearby tests that do not prove the exact subcase.

## Active Sources Of Truth

- Active milestone plan: `docs/plans/remaining-partial-feature-milestones.md`.
- Partial row inventory: `docs/plans/partial-feature-inventory.md`.
- Feature status: `docs/stab-feature-checklist.md`.
- Stim feature inventory: `docs/stim-feature-list.md`.
- Historical roadmap and milestone policy: `docs/plans/rust-stim-drop-in-rewrite.md`.
- Test-porting map: `docs/plans/stim-test-porting-plan.md`.
- Spec-gap log: `docs/plans/milestone-spec-gaps.md`.
- Upstream baseline: pinned Stim v1.16.0 in `vendor/stim`.
- Oracle source of truth: `oracle/fixtures/manifest.csv`.
- Benchmark source of truth: `benchmarks/manifest.csv`, `benchmarks/m12-primary-thresholds.json`, `benchmarks/m12-primary-beta-waivers.json`, and `benchmarks/profiler-notes/`.

If these files disagree, fix the stale source before implementing or claiming completion.

## Scope

Included:

- RPF0 inventory and comparator lock.
- RPF1 gate metadata and gate execution support contracts.
- RPF2 circuit transforms and feedback-inlining transforms.
- RPF3 sweep-conditioned execution and legal-gate execution gaps.
- RPF4 DEM APIs, transforms, coordinates, counts, and folded traversal.
- RPF5 detector utility APIs and measurement-rich flows.
- RPF6 analyzer, search, sparse reverse tracking, and active matched-error value-object hardening.
- RPF7 visible CLI parity for `m2d`, `analyze_errors`, and accepted legacy aliases.
- RPF8 benchmark-gate, audit, review, and documentation closure.

Excluded:

- Python bindings and Python API clone behavior.
- JavaScript/WASM.
- Diagrams and visualization.
- `stim explain_errors` CLI.
- `stim repl`.
- QASM, Quirk, Crumble, ecosystem packages, GPU backends, exact random-stream parity, and C++ header compatibility.
- New public graph simulator, vector simulator, `TableauSimulator`, or `FlipSimulator` products.
- Deprecated `--detector_hypergraph` support.

Do not silently widen scope into an excluded surface.
When an active subcase depends on excluded work, log the under-specification in `docs/plans/milestone-spec-gaps.md` and keep the feature partial or deferred.

## Milestone Work Loop

For every RPF milestone:

1. Re-read the owned milestone section in `docs/plans/remaining-partial-feature-milestones.md`.
2. Reconcile the milestone with `docs/stab-feature-checklist.md`, `docs/plans/partial-feature-inventory.md`, `docs/stim-feature-list.md`, pinned Stim source, `docs/plans/lessons-learned.md`, and `docs/plans/milestone-spec-gaps.md`.
3. Write or refresh a scope note before coding that names owned subcases, deferred subcases, unsupported shapes, comparator class, oracle rows, benchmark rows, and resource behavior.
4. Add or port targeted tests before or alongside implementation.
5. Implement the feature with existing Stab abstractions, typed boundaries, clear domain errors, and streaming or documented caps for public IO paths.
6. Run targeted tests while iterating and fix failures.
7. Add or update oracle rows, benchmark rows, measurement work units, compare notes, profiler notes, waivers, threshold entries, and fixture metadata when the milestone requires them.
8. Update docs in the same change set, including the checklist, active plan, milestone progress or completion report, roadmap text, oracle metadata, benchmark metadata, and user-facing docs when behavior changes.
9. Run milestone-audit and fix implementation, evidence, test, benchmark, or documentation findings.
10. Run full-code-review for touched surfaces and fix findings; when using Codex, spawn GPT-5.5/xhigh subagents during the review.
11. Log under-specified scope in `docs/plans/milestone-spec-gaps.md` instead of hiding it in checklist wording.

A milestone is incomplete if any item in this loop is missing.

## Tests Required By Milestone

- RPF0: `cargo test -p stab-oracle fixtures --quiet`, `just oracle::list`, `just oracle::matrix --check`, and `just bench::list`.
- RPF1: gate metadata tests, gate execution support table tests, unsupported metadata tests, and any gate benchmark metadata tests.
- RPF2: circuit transform tests for `flattened`, `flattened_operations`, `without_noise`, `decomposed`, feedback inlining, repeat traversal, semantic preservation, and resource caps.
- RPF3: sweep-conditioned sampler, converter, detector, analyzer, and CLI tests across accepted formats, omitted defaults, width mismatches, unsupported sweep shapes, and legal-gate execution boundaries.
- RPF4: DEM exact, structural, negative, and resource-boundary tests for `flattened`, `rounded`, `without_tags`, coordinates, counts, final shifts, repeats, and folded traversal consumers.
- RPF5: detecting-region, missing-detector, measurement-rich flow, flow validation, failure-diagnostic, and transform-integration tests for every promoted utility subfamily.
- RPF6: analyzer, generated-circuit, decomposition, gauge, loop-folding, search, sparse reverse tracking, and active matched-error value-object tests using exact or structural comparators as appropriate.
- RPF7: CLI tests and oracle rows for accepted flags, rejected flags, path errors, writer errors, stdout behavior, stderr class, exit status, input formats, output formats, side outputs, resource boundaries, accepted legacy aliases, conflicts, and `--detector_hypergraph` exclusion.
- RPF8: benchmark, oracle, audit, review, documentation, and checklist consistency checks for every completed milestone slice.

Use narrower filters during iteration, but do not claim milestone completion from tests that only cover constants, static labels, or broad smoke behavior.

## Benchmark Rules

Every performance-sensitive milestone must have benchmark metadata before completion.

For each benchmark row:

- Classify it as `direct-match`, `cli-baseline`, `contract-representative`, `contract-proxy`, `contract-smoke`, `partial-match`, `report-only`, or `contract-only`.
- Define measurement work units before collecting evidence.
- Add compare notes explaining whether pinned Stim is a faithful baseline.
- Add runner coverage or keep the row as an explicit placeholder.
- Keep report-only, proxy, tiny, partial-match, and no-ratio rows out of the 1.25x primary threshold file unless a source-owned waiver and repeated evidence justify otherwise.
- Use schema-version-2 submeasurement thresholds when one row bundles stable and unstable submeasurements.
- Update profiler notes in the same change set as threshold changes.

Fresh primary benchmark evidence for newly gated rows must come from current committed `HEAD` or an explicitly recorded local-modification state:

```sh
just bench::baseline --primary --out target/benchmarks/remaining-partial-primary-baseline
just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --baseline target/benchmarks/remaining-partial-primary-baseline/baseline.json --report target/benchmarks/remaining-partial-primary-compare
just bench::primary-regression --baseline target/benchmarks/remaining-partial-primary-baseline/baseline.json --report target/benchmarks/remaining-partial-primary-regression
just bench::primary-memory-regression --baseline target/benchmarks/remaining-partial-primary-baseline/baseline.json
```

Do not cite exploratory probes as release evidence.

## Documentation Rules

When behavior changes, update documentation in the same change set.

At minimum, check whether the change affects:

- `docs/stab-feature-checklist.md`.
- `docs/plans/remaining-partial-feature-milestones.md`.
- The matching milestone progress or completion report in `docs/plans/`.
- `docs/plans/rust-stim-drop-in-rewrite.md`.
- `docs/plans/stim-test-porting-plan.md`.
- `docs/plans/partial-feature-inventory.md`.
- `docs/plans/milestone-spec-gaps.md`.
- `README.md` or CLI docs.
- Oracle manifests, benchmark manifests, threshold files, waivers, profiler notes, and fixture metadata.

Checklist rows may move from `Partial` to `Done` only after the owned subcases, tests, benchmarks, audits, and review findings prove the claim.
Rollup rows may move only after every active child row is implemented or explicitly deferred with a named reason.

## Final Verification

Before claiming the full goal complete, run:

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings
cargo test --workspace --quiet
just oracle::run --implemented-only
just bench::smoke
just maintenance::pre-commit
```

If the work changes benchmark gates, also run the primary benchmark commands in the benchmark rules section.
If the work changes shared parser, sampler, analyzer, stabilizer, oracle, benchmark, or CLI infrastructure, expand verification beyond the targeted milestone filters.

## Completion Criteria

The goal is complete only when:

- Every non-deferred partial row in `docs/stab-feature-checklist.md` has implemented evidence or remains partial with a named deferred subcase.
- Every rollup row is backed by child-row evidence and does not imply broader parity than the implemented surfaces prove.
- Every implemented feature has meaningful targeted tests, including positive, negative, compatibility, and resource-boundary cases where relevant.
- Every public CLI behavior has CLI tests or oracle rows proving accepted flags, rejected flags, input and output formats, stdout behavior, stderr class, exit status, path handling, writer behavior, and resource behavior.
- Every public Rust API added by the plan uses typed domain values and clear domain errors after external boundaries.
- Every benchmarked feature has source-owned manifest metadata, runner coverage or explicit placeholder status, measurement work, compare notes, and a comparability class.
- Every primary-gated benchmark row has repeated stable comparable evidence and matching profiler notes.
- Report-only rows are labeled as report-only and are not used as release gates.
- `docs/stab-feature-checklist.md`, active plan docs, roadmap docs, oracle metadata, benchmark metadata, threshold files, waivers, profiler notes, and milestone reports agree with current behavior.
- Milestone-audit and full-code-review findings are fixed, or under-specification findings are logged.

## Stop Conditions

Stop and write a spec-gap entry instead of coding when:

- A subcase requires an excluded surface.
- A milestone still depends on a whole upstream file instead of exact owned subcases.
- A public CLI behavior cannot define accepted flags, rejected flags, input formats, output formats, stdout behavior, stderr class, exit status, path handling, and resource behavior.
- A benchmark row cannot be classified or cannot produce faithful comparable evidence.
- A completion claim would require stale reports, informal waivers, or unrecorded local modifications.
- A public parser, converter, sampler, analyzer, transformer, search, or writer path has neither streaming behavior nor a documented cap.
- A checklist update would need to overstate implemented behavior to mark a row done.
