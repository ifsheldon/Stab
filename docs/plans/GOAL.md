# Goal: Finish Non-Deferred Partial Feature Milestones

## Mission

Finish the active milestones in `docs/plans/non-deferred-partial-feature-milestones.md`.
The objective is to close every `Partial` row in `docs/stab-feature-checklist.md` whose remaining work is not intentionally deferred, while keeping deferred Python, JS/WASM, diagram, ecosystem, simulator-product, GPU, exact-randomness, C++ header, and deprecated `--detector_hypergraph` surfaces out of scope.

Use `docs/plans/lessons-learned.md` before planning or implementing each milestone.
The recurring failure to avoid is claiming completion from broad checklist wording, broad upstream test files, stale reports, report-only benchmarks, or nearby tests that do not prove the exact subcase.

## Active Sources Of Truth

- Active execution plan: `docs/plans/non-deferred-partial-feature-milestones.md`.
- Feature status: `docs/stab-feature-checklist.md`.
- Stim feature inventory: `docs/stim-feature-list.md`.
- Partial-row extraction map: `docs/plans/partial-feature-inventory.md`.
- Historical RPF plan and reports: `docs/plans/remaining-partial-feature-milestones.md` and `docs/plans/rpf*-*.md`.
- Current PFM8 rollup evidence report: `docs/plans/pfm8-rollup-evidence-report.md`.
- Historical roadmap and benchmark policy: `docs/plans/rust-stim-drop-in-rewrite.md`.
- Test-porting map: `docs/plans/stim-test-porting-plan.md`.
- Lessons and known spec gaps: `docs/plans/lessons-learned.md` and `docs/plans/milestone-spec-gaps.md`.
- Upstream baseline: pinned Stim v1.16.0 in `vendor/stim`.
- Oracle metadata: `oracle/fixtures/manifest.csv`.
- Benchmark metadata: `benchmarks/manifest.csv`, `benchmarks/m12-primary-thresholds.json`, `benchmarks/m12-primary-beta-waivers.json`, and `benchmarks/profiler-notes/`.

If these files disagree, fix the stale source before implementing or claiming completion.

## Work Loop For Each Milestone

For every PFM milestone:

1. Re-read the milestone section in `docs/plans/non-deferred-partial-feature-milestones.md`.
2. Reconcile the milestone with the checklist, partial inventory, Stim feature inventory, pinned Stim source, lessons learned, and spec-gap log.
3. Write or refresh a scope note before coding that names owned subcases, explicit rejections, explicit deferrals, comparator class, oracle rows, benchmark rows, resource behavior, and public API or CLI shape.
4. Port or create meaningful tests before or alongside implementation.
5. Implement the feature using existing Stab abstractions, typed boundaries, clear domain errors, and streaming or documented caps for public IO paths.
6. Run targeted tests while iterating and fix failures.
7. Add or update oracle rows, benchmark rows, measurement work units, compare notes, profiler notes, waivers, threshold entries, and fixture metadata when the milestone requires them.
8. Update documentation in the same change set, including the checklist, active plan, milestone report, roadmap text, oracle metadata, benchmark metadata, and user-facing docs when behavior changes.
9. Run milestone-audit and fix implementation, evidence, test, benchmark, or documentation findings.
10. Run full-code-review for touched surfaces and fix findings; when using Codex, spawn GPT-5.5/xhigh subagents during the review.
11. Log true under-specification in `docs/plans/milestone-spec-gaps.md` instead of hiding it in checklist wording.

A milestone is incomplete if any item in this loop is missing.

## Test Rules

Use the milestone-specific tests listed in `docs/plans/non-deferred-partial-feature-milestones.md`.
Tests must protect behavior, compatibility, resource boundaries, typed errors, malformed inputs, CLI stdout and stderr behavior, exit status, and unsupported-shape handling where relevant.
Do not count tests that only check constants, static labels, or broad smoke behavior as completion evidence.

During iteration, use focused filters.
Before claiming milestone completion, run the milestone’s full targeted test set plus any oracle, benchmark, and documentation checks named by the plan.

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
just bench::baseline --primary --out target/benchmarks/non-deferred-partials-primary-baseline
just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --baseline target/benchmarks/non-deferred-partials-primary-baseline/baseline.json --report target/benchmarks/non-deferred-partials-primary-compare
just bench::primary-regression --baseline target/benchmarks/non-deferred-partials-primary-baseline/baseline.json --report target/benchmarks/non-deferred-partials-primary-regression
just bench::primary-memory-regression --baseline target/benchmarks/non-deferred-partials-primary-baseline/baseline.json
```

Do not cite exploratory probes as release evidence.

## Documentation Rules

When behavior changes, update documentation in the same change set.
At minimum, check whether the change affects:

- `docs/stab-feature-checklist.md`.
- `docs/plans/non-deferred-partial-feature-milestones.md`.
- `docs/plans/partial-feature-inventory.md`.
- The matching milestone progress or completion report in `docs/plans/`.
- `docs/plans/rust-stim-drop-in-rewrite.md`.
- `docs/plans/stim-test-porting-plan.md`.
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
- Checklist, active plan docs, roadmap docs, oracle metadata, benchmark metadata, threshold files, waivers, profiler notes, and milestone reports agree with current behavior.
- Milestone-audit and full-code-review findings are fixed, or true under-specification findings are logged.

## Stop Conditions

Stop and write a spec-gap entry instead of coding when:

- A subcase requires an excluded surface.
- A milestone still depends on a whole upstream file instead of exact owned subcases.
- A public CLI behavior cannot define accepted flags, rejected flags, input formats, output formats, stdout behavior, stderr class, exit status, path handling, and resource behavior.
- A benchmark row cannot be classified or cannot produce faithful comparable evidence.
- A completion claim would require stale reports, informal waivers, or unrecorded local modifications.
- A public parser, converter, sampler, analyzer, transformer, search, or writer path has neither streaming behavior nor a documented cap.
- A checklist update would need to overstate implemented behavior to mark a row done.

## Commit Policy

Do not commit solely because this goal exists.
When the current thread explicitly authorizes commits, use focused commits following the repository commit-message convention and run the required targeted verification before committing.
