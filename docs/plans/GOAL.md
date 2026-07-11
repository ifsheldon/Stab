# Goal: Resolve The Remaining Non-Deferred Blockers

## Mission

Finish the blocker closure program in `docs/plans/non-deferred-partial-feature-milestones.md`.
The objective is to resolve all eight open entries in `docs/plans/milestone-spec-gaps.md`, close every remaining active Rust or CLI feature gap, and stop treating intentionally deferred products or unbounded words such as `full`, `broader`, and `every` as implementation instructions.

The required outcome is not another collection of selected fixture-shaped handlers.
The required outcome is a maintainable semantic foundation with exhaustive support contracts, shared traversal and flow engines, generic loop folding, exact source-owned tests, honest benchmark classifications, and synchronized status documentation.

Read `docs/plans/lessons-learned.md` before starting each milestone.
The central lesson is that ambiguity must be removed before coding and that nearby tests, broad upstream files, stale reports, and report-only benchmarks do not prove a feature claim.

## Active Sources Of Truth

- Execution plan: `docs/plans/non-deferred-partial-feature-milestones.md`, especially PFM-B0 through PFM-B6.
- Executable closure ledger: `docs/plans/blocker-closure-ledger.json`, validated with `just oracle::blockers --check-selectors`.
- Open blocker log: `docs/plans/milestone-spec-gaps.md`.
- Current rollup evidence: `docs/plans/pfm8-rollup-evidence-report.md`.
- Feature status: `docs/stab-feature-checklist.md`.
- Stim inventory: `docs/stim-feature-list.md`.
- Partial-row map: `docs/plans/partial-feature-inventory.md`.
- Lessons learned: `docs/plans/lessons-learned.md`.
- Historical roadmap: `docs/plans/rust-stim-drop-in-rewrite.md`.
- Test-porting map: `docs/plans/stim-test-porting-plan.md`.
- Frozen oracle: Stim v1.16.0 in `vendor/stim`.
- Oracle metadata: `oracle/fixtures/manifest.csv`.
- Benchmark metadata: `benchmarks/manifest.csv`, `benchmarks/m12-primary-thresholds.json`, `benchmarks/m12-primary-beta-waivers.json`, and `benchmarks/profiler-notes/`.

If these sources disagree, fix the disagreement before implementation continues.

## Current Checkpoint

PFM-B0 is complete as of 2026-07-10.
PFM-B2 contract groundwork is also complete: canonical metadata now classifies all 81 gates across eight surfaces and maps parser-accepted target groups, including anti-Hermitian Pauli products, to typed behavior or rejection decisions with no unknown state.
Ledger schema version 2 requires all eight surfaces per gate-family case, validates its surface and family names against canonical core metadata, and gives every one of the nineteen semantic families an explicit owner. Deterministic MPP, anti-Hermitian MPP rejection, deterministic MPAD, stochastic MPP, and stochastic MPAD now have separate provenance and comparator records; eighteen cases remain planned until final PFM-B2 coverage after the shared foundations stabilize.
PFM-B3 is complete as of 2026-07-10: all seven ledger cases use the shared folded traversal or the documented direct compact-transform path, have independent tests and oracle rows, and have clean committed-HEAD allocation evidence from `4a984c26b39f6236fde5e3ff10cf0b42e8b155a2`. Its milestone-audit and GPT-5.6/max review findings are closed, with only non-blocking future ledger-schema hardening logged.
PFM-B4 is complete as of 2026-07-11 at `0f47eee04eacec96ed4e03dd36a18f58b76a0afc`: all forty-nine cases have exact one-test selectors, detecting-region and missing-detector evidence is focused, the flow engine is `33/33 implemented`, the exhaustive solver fallback is removed, generator plus sparse-checker dispatch share typed reverse-transition classification, all milestone-audit and GPT-5.6/max findings are closed, and both required allocation reports record `local_modifications=false` with zero resident delta. PFM-B1 is active with all nineteen cases implemented and every known review finding fixed locally: reverse-flow state is sparse, high-index unitary validation is memory-budgeted, empty nested repeats and empty checker batches skip unnecessary work, width-mismatched idle-qubit flows use sparse validation, output validation is batched, reversal-only record aliases match Stim rejection without changing ordinary checker XOR semantics, observable effects combine before collapse checks, exact goldens re-record from pinned C++ Stim with digest bindings, truncated, symlink-routed, or hard-link-targeting evidence operations fail closed, committed fixture, child-output, helper-protocol, and compatibility-matrix inputs are bounded, oversized live side outputs terminate their process group, per-run scratch directories clean up through RAII, ignored tests cannot satisfy evidence, and allocation contracts use checked incremental-slope gates where appropriate. Finish final verification, the implementation commit, and clean committed-HEAD allocation reports before advancing to PFM-B5; do not reopen PFM-B0, PFM-B2 contract groundwork, PFM-B3, or PFM-B4 unless their frozen contracts change.

## Scope Decisions

The following decisions are fixed for this goal:

- Implement general reverse-flow and QEC transform semantics for the selected Rust APIs.
- Keep broader repeat-contained feedback-inlining beyond the already selected bounded loop cases deferred unless a later plan explicitly reopens it.
- Evidence-close analyzer sweep behavior at the current pinned matrix instead of inventing additional sweep shapes.
- Implement an exhaustive gate-by-surface semantic contract for legal execution behavior.
- Implement one shared folded DEM traversal for bounded-result consumers.
- Evidence-close detecting regions and missing detectors from their complete named pinned subcases plus existing promoted evidence.
- Implement general GF(2) flow solving and shared stabilizer-flow transitions.
- Implement generic analyzer loop-state cycle folding and finite graphlike, hypergraph, and SAT/WCNF closure corpora.
- Keep full ErrorMatcher provenance and `explain_errors` deferred.

The following surfaces remain excluded: Python bindings, JS/WASM, diagrams, `repl`, QASM, Quirk, Crumble, ecosystem packages, GPU, exact random-stream parity, public graph or vector simulator products, C++ header compatibility, and deprecated `--detector_hypergraph` behavior.

An excluded surface must not keep an otherwise completed Rust or CLI child row marked as an active blocker.

## Required Execution Order

Execute the blocker milestones in this order:

1. PFM-B0: freeze the subcase-level closure ledger.
2. PFM-B2 contract groundwork: add the gate-by-surface classification before changing shared execution semantics.
3. PFM-B3: add shared folded DEM traversal before migrating analyzer, search, SAT, or matcher consumers.
4. PFM-B4 flow foundation: replace exhaustive flow solving and establish shared stabilizer transitions.
5. PFM-B1: finish reverse-flow and QEC transforms using the shared flow foundation.
6. PFM-B5: implement generic analyzer loop folding and finish search, SAT, sparse-tracker, and active matched-error closure.
7. Finish the PFM-B2 generated semantic matrix after the shared engines stabilize.
8. PFM-B6: resolve spec gaps, run audits and review, and roll up statuses.

PFM-B4 detecting-region and missing-detector evidence closure may run in parallel with PFM-B3 because it must not add speculative behavior.
Do not begin PFM-B6 while any owned blocker row lacks executable evidence.

## Work Loop For Every Milestone

For each PFM-B milestone:

1. Re-read the exact milestone section and its owned blocker entries.
2. Reconcile the closure ledger with pinned Stim source, the checklist, partial inventory, lessons learned, oracle manifest, and benchmark manifest.
3. Write or refresh a scope or progress report naming every owned subcase, explicit rejection, deferral, comparator, public surface, resource contract, oracle row, benchmark row, and done criterion.
4. Add or port meaningful tests before or alongside production changes.
5. Implement the general abstraction named by the milestone and migrate the owned consumers.
6. Run focused tests during iteration and fix semantic, error, and resource regressions.
7. Update oracle rows and benchmark metadata in the same change set as behavior or performance-path changes.
8. Update all affected documentation in the same change set.
9. Run milestone-audit and fix every implementation, evidence, benchmark, test, and documentation finding.
10. Log genuine newly discovered under-specification in `docs/plans/milestone-spec-gaps.md`; do not use a new gap entry to avoid a decision already made by this goal.
11. Run full-code-review over the touched surfaces and fix findings.

A milestone is incomplete if any step is missing.

## Architecture Guardrails

- Do not add production code that recognizes fixture text, exact instruction sequences, exact recurrence periods, exact detector counts, or one generated circuit signature.
- Gate semantics must be expressed by gate-family and target-role transitions.
- Flow generation, flow checking, flow solving, time reversal, detecting regions, missing detectors, and sparse reverse tracking should share stabilizer transition logic where their semantics overlap.
- Flow solving must use GF(2) elimination or an equivalently general polynomial algorithm, not exhaustive subset enumeration.
- Folded DEM traversal must carry checked detector and coordinate shifts through nested repeats and support early termination.
- Analyzer loop folding must identify recurrence from canonical boundary state, not hard-coded periods such as 8 or 127.
- A public materializing API may retain a documented cap when its result is inherently expanded.
- A bounded-result internal consumer may not flatten repeated input merely for inspection.
- Search may retain documented limits for genuinely exponential state spaces, but traversal limits and search-complexity limits must be distinct and tested.
- Unsupported runtime shapes must return precise typed errors and must not silently skip work or substitute fallback semantics.

If implementation cannot satisfy these guardrails, stop and revise the milestone contract before adding another special case.

## Test Rules

- Every blocker ledger row must name a planned test selector, and every row claimed as implemented or evidence-closed must select an executable Rust test or explicit evidence-close check.
- Multi-example upstream tests must be split into stable subcase ids before they count as complete.
- PFM-B0 may record a selector shared by several stable subcase ids only as explicit evidence-splitting debt; the owning implementation or evidence-close milestone cannot complete until every owned case has an independently selectable test.
- Use exact comparators for canonical circuit text, deterministic DEM text, stable WCNF text, deterministic flow lists, and CLI output where order is contractual.
- Use structural comparators for detecting-region maps, flow spans, and tie-sensitive logical-error target sets.
- Use statistical comparators only for probabilistic behavior, with source-owned shot counts, seeds, tolerances, bucket definitions, and false-positive budgets.
- For blocker-ledger statistical plans, compare each observed bucket probability against its pinned expectation using `max(absolute_probability_floor, sigma_multiplier * sqrt(p * (1 - p) / shots))`; apply the declared familywise false-positive budget across all named buckets, and reject a plan during its owner milestone if an exact binomial-tail check shows that the declared budget is not met.
- Add positive, negative, malformed-input, overflow, unsupported-shape, visitor-error, and resource-boundary tests for each relevant public or internal surface.
- Add small generated differential tests whenever a folded or optimized path has a straightforward materialized reference implementation.
- Property tests must use deterministic seeds and print enough minimized context to reproduce failures.

Do not count tests that only restate metadata constants, static labels, or freshly constructed fields.

## Benchmark Rules

Every performance-sensitive milestone must update benchmark metadata before completion.

For every changed or added row:

- Assign a comparability class before timing it.
- Define work units that reflect the algorithm, such as matrix bits or pivots, DEM items visited, represented repeat iterations, analyzer states, search nodes, clauses, or circuit operations.
- Record peak live allocation and sampled resident delta for traversal, loop-folding, and large matrix workloads.
- Add compare notes explaining whether pinned Stim is a faithful baseline.
- Keep report-only, proxy, partial-match, tiny, and no-ratio rows out of the 1.25x threshold gate.
- Use schema-version-2 submeasurement thresholds when one row contains different algorithms or size classes.
- Update profiler notes and threshold entries in the same change set.

New primary-gate evidence must use a fresh baseline from current committed `HEAD`:

```sh
just bench::baseline --primary --out target/benchmarks/blocker-closure-primary-baseline
just bench::compare --primary --warmup --measurement-runs 3 --require-profiler-notes --baseline target/benchmarks/blocker-closure-primary-baseline/baseline.json --report target/benchmarks/blocker-closure-primary-compare
just bench::primary-regression --baseline target/benchmarks/blocker-closure-primary-baseline/baseline.json --report target/benchmarks/blocker-closure-primary-regression
just bench::primary-memory-regression --baseline target/benchmarks/blocker-closure-primary-baseline/baseline.json
```

Exploratory probes are not completion evidence.

## Documentation Rules

Every milestone must check and update, where applicable:

- `docs/stab-feature-checklist.md`.
- `docs/plans/non-deferred-partial-feature-milestones.md`.
- `docs/plans/milestone-spec-gaps.md`.
- `docs/plans/partial-feature-inventory.md`.
- The matching scope and progress report.
- `docs/plans/rust-stim-drop-in-rewrite.md`.
- `docs/plans/stim-test-porting-plan.md`.
- `docs/plans/pfm8-rollup-evidence-report.md`.
- `README.md` and CLI or Rust API docs when public behavior changes.
- Oracle and benchmark manifests, thresholds, waivers, profiler notes, and fixture metadata.

Use `Done for selected Rust/CLI scope` when the active child surface is complete but literal full-Stim product parity still depends on deferred products.
Do not leave such a row `Partial` and call it an active blocker.

## Milestone Completion Criteria

Each implementation milestone is complete only when:

- Every owned subcase has implementation or evidence-close status and direct executable evidence.
- Every promoted public behavior has a defined comparator, error contract, and resource contract.
- Every unsupported shape fails closed with a precise error or is outside the selected public surface.
- No production implementation depends on fixture signatures or hard-coded recurrence periods.
- Tests cover correctness, compatibility, negative behavior, and resource boundaries.
- Benchmark metadata and evidence match the milestone's performance claims.
- Documentation agrees with current behavior.
- Milestone-audit and full-code-review findings are fixed.

## Goal Completion Criteria

The whole goal is complete only when:

- All eight open spec-gap entries are resolved.
- PFM3 analyzer sweep, PFM5 detecting regions, and PFM5 missing detectors are evidence-closed without speculative scope growth.
- The gate-by-surface contract covers every canonical gate and relevant implemented surface with no unknown classification.
- Every legal target-role shape inside the selected Rust or CLI surface executes, lowers, or has defined no-op or annotation semantics; unsupported classifications are limited to invalid combinations or named exclusions and cannot satisfy a full-semantic status claim.
- Reverse-flow transforms use general gate-family transitions and pass the owned pinned and generated corpus.
- Flow solving has no exhaustive-subset size cliff.
- Bounded-result DEM consumers use shared folded traversal or have a precise inherent-materialization rationale.
- Analyzer loop folding has no fixture-specific or hard-coded-period production path.
- Named graphlike, hypergraph, and SAT/WCNF corpora have exact or structural parity evidence.
- Remaining caps protect inherent output or algorithmic complexity and have explicit tests.
- Active checklist child rows are no longer generically partial.
- Deferred product surfaces remain explicit and do not masquerade as active blockers.
- Final PFM8 evidence comes from current `HEAD` and contains audit, review, tests, oracle, benchmark, and documentation closure.

## Final Verification

Run before claiming the goal complete:

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings
cargo test --workspace --quiet
just oracle::blockers --check-selectors
just oracle::run --implemented-only
just bench::smoke
just maintenance::pre-commit
```

Run the primary benchmark commands when any primary runner, threshold, or shared hot path changes.

## Stop Conditions

Stop and amend the plan when:

- An owned subcase still depends on a whole upstream file instead of stable subcase ids.
- Expected behavior cannot be established from pinned Stim, a documented Stab hardening decision, or a precise invariant.
- A proposed fix requires a fixture-shaped branch or hard-coded recurrence period.
- A public parser, transformer, converter, sampler, analyzer, search, or writer path has neither streaming or folded behavior nor a documented cap.
- A benchmark row cannot be classified or cannot define meaningful work units.
- A performance claim depends on stale artifacts, unrecorded local modifications, missing profiler notes, or an informal waiver.
- A checklist status would overstate Python, JS/WASM, diagram, ecosystem, simulator-product, or provenance parity.

## Commit Policy

Do not commit solely because this goal exists.
When a thread explicitly authorizes commits, group changes by blocker milestone, use the repository commit-message convention, and run the milestone's targeted verification before each focused commit.
