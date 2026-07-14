# Goal: Qualify Stab Correctness And Performance Against Stim

## Status

Active execution goal as of 2026-07-14.

The previous non-deferred blocker rollup is complete and recorded in `docs/plans/pfm8-rollup-evidence-report.md`.
This goal replaces that completed execution contract; it does not reopen the finite PFM-B ledger.

## Mission

Implement the comprehensive correctness qualification suite in `docs/plans/comprehensive-correctness-qualification-plan.md` and the comprehensive pinned-Stim performance qualification suite in `docs/plans/comprehensive-stim-performance-qualification-plan.md`.

The result must make current Stab claims auditable at case level and performance claims reproducible at equivalent-work level.
It must expose missing, failed, deferred, noisy, slow, and no-faithful-comparator outcomes honestly instead of turning broad green commands or aggregate medians into evidence.

Read `docs/plans/lessons-learned.md` before starting a milestone and again before accepting it.
The plans were written to prevent the specific failures recorded there: file-level acceptance, vague partial scope, implicit comparators, CLI/core conflation, untested resource limits, weak benchmark classes, stale evidence, noisy tiny timings, heterogeneous medians, unchecked waivers, missing review closure, and documentation drift.

## Active Sources Of Truth

- Correctness execution and acceptance: `docs/plans/comprehensive-correctness-qualification-plan.md`.
- Active CQ2 execution evidence: `docs/plans/cq2-deterministic-qualification-progress-report.md`.
- Completed CQ1 harness evidence: `docs/plans/cq1-correctness-harness-progress-report.md`.
- Performance execution and acceptance: `docs/plans/comprehensive-stim-performance-qualification-plan.md`.
- Completed PQ1 harness evidence: `docs/plans/pq1-performance-harness-progress-report.md`.
- Implemented and deferred feature boundary: `docs/stab-feature-checklist.md`.
- Upstream feature inventory: `docs/stim-feature-list.md`.
- Historical upstream test hierarchy: `docs/plans/stim-test-porting-plan.md`.
- Frozen compatibility target: Stim v1.16.0 at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07` in `vendor/stim`.
- Existing oracle evidence: `oracle/compatibility-matrix.csv`, `oracle/fixtures/manifest.csv`, and `docs/plans/blocker-closure-ledger.json`.
- Existing benchmark evidence: `benchmarks/manifest.csv`, M12 threshold and waiver files, and source-owned benchmark reports.
- Under-specification record: `docs/plans/milestone-spec-gaps.md`.
- Planning lessons: `docs/plans/lessons-learned.md`.

If these sources disagree, stop the affected acceptance claim and fix the disagreement before continuing.
Do not choose whichever source makes completion easiest.

## Scope

Qualify every implemented, non-deferred Rust and CLI contract selected by `docs/stab-feature-checklist.md` and every exported Rust API item that implements those selected contracts.
When a checklist row is partial, split it into exact implemented children and exact unimplemented or deferred children before assigning evidence.
Only the implemented children enter executable qualification; the remaining children must stay visible and cannot be counted as passes.

This goal does not implement Python bindings, JS/WASM, diagrams, `repl`, QASM, Quirk, Crumble, ecosystem packages, GPU, exact Stim random streams, public interactive graph or vector simulators, full ErrorMatcher provenance, C++ header compatibility, or other explicitly deferred products.
Do not expand product scope while building qualification infrastructure.

## Program Order

Program checkpoint: CQ0 is current at correctness inventory digest `adcc3d19605e4fc7bd9e1b3f3373ddf38d81301430d891f062baaab0c6fdc8f6`; PQ0 is dependency-regenerated at performance inventory digest `3b55089bc331b115912818ba04b584c741a8c9a56e13d8b06d85cc829b6256a1`. Clean result-format CQ1 evidence from revision `2f7e456b43415303eef19c3d2850811fa9b2526d` passes PR 31/31, full 39/39, and soak 39/39 with offline report regeneration and exact full preflight. The prior PQ1 reports remain valid historical harness evidence but must be refreshed against the new performance digest before being cited as current.
The active milestone is CQ2: qualify deterministic models, formats, gates, bit kernels, and stabilizer algebra at exact case and exported-API granularity before PQ2 times those surfaces. The selected `.stim`, `.dem`, and result-format domains are complete with 32, 28, and 39 implemented evidence parents respectively and zero planned owners. The result-format slice comprises 36 qualification parents and three direct oracle-fixture parents; it assigns all 156 selected upstream records and 97 selected exported Rust API items without promoting seven broad fixtures beyond supporting provenance. CQ2 now has 240 implemented owners and 2,065 planned owners across 2,305 selected owners, so continue with `CQ-GATE-CONTRACT`, where 646 of 730 owners remain planned, instead of treating the completed format slices as CQ2 completion.
Keep PQ1's `pq1-adapter-protocol-smoke` ratio permanently diagnostic and never report it as product speed evidence.
Do not reopen CQ0 or PQ0 inventory semantics unless pinned-source drift, a newly exported default-feature API, a stale referenced id, a changed checklist or benchmark source of truth, or a confirmed inventory defect changes a frozen digest.
Do not treat PQ0's 15 retained rows as qualified evidence: all 159 active inherited rows still record missing correctness preflight and output-digest evidence, 58 CLI rows remain asymmetric, 73 rows lack a current comparator, 124 lack required scale families, and 21 select heterogeneous upstream measurements.

Execute the milestones in this dependency order:

1. Complete CQ0 to freeze case-level correctness ids and upstream dispositions.
2. Complete PQ0 using the frozen CQ0 feature and correctness ids.
3. Keep completed CQ1 and PQ1 evidence valid while adding product cases and runtime groups.
4. Complete CQ2, then PQ2 for deterministic models, formats, gates, kernels, and algebra.
5. Complete CQ3, then PQ3 for public CLI, generation, conversion, and startup.
6. Complete CQ4, then PQ4 for sampling, detection, conversion, and DEM sampling.
7. Complete CQ5, then PQ5 for analysis, search, flows, utilities, and transforms.
8. Complete CQ6 to close selected correctness qualification.
9. Complete PQ6 to graduate memory, scaling, and stable timing thresholds.
10. Complete PQ7 to run final performance qualification and synchronize the program report.

PQ0 may classify proposed benchmark groups before all CQ cases are implemented, but no timing row may become `qualified`, enter a 1.25x gate, or create source-owned ratio evidence until every referenced CQ case passes.
Work on independent implementation modules may proceed in parallel only when each milestone retains a single owner for its inventory, reports, and acceptance state.

## Milestone Execution Contract

For every CQ and PQ milestone:

1. Re-read the milestone objective, tasks, tests, and acceptance criteria.
2. Inspect the current code, upstream Stim source and tests, existing Stab evidence, and relevant feature-checklist rows before editing.
3. Add or update the machine-readable inventory first so the finite selected subcases and expected evidence are explicit.
4. Port or create independently selectable tests before changing production behavior or claiming coverage.
5. Confirm that each new test fails for the intended missing behavior, missing harness capability, or deliberately injected bad fixture when practical.
6. Implement only the behavior, harness, adapter, fixture, or report support selected by the milestone.
7. Run targeted tests during iteration and the milestone's full done commands before acceptance.
8. Run the `milestone-audit` skill against objective, tasks, tests, benchmarks, resource contracts, linked evidence, and done criteria.
9. Fix every audit implementation, correctness, evidence, resource, benchmark, and documentation issue.
10. Log only genuine newly revealed under-specification in `docs/plans/milestone-spec-gaps.md`; do not use an entry to postpone an already required decision.
11. Run the `full-code-review` skill over all touched Rust, CLI, oracle, adapter, file-format, hostile-input, SIMD, performance, test, operational, and documentation surfaces.
12. Fix every confirmed review finding and rerun affected checks.
13. Write a concise milestone progress report under `docs/plans/` with exact inventory counts, commands, results, artifact paths, clean-revision metadata where applicable, audit status, review status, remaining failures, and deferrals.
14. Synchronize the checklist, test-porting plan, benchmark docs, roadmap, manifests, schemas, and command docs in the same change set when their contracts change.

Do not mark all checklist items complete at the end without updating them as evidence lands.
Do not make a commit unless the user explicitly asks for one.
When a commit is requested, use focused commits and run the repository's required verification first.

## Correctness Rules

- Every selected case needs a stable id and an independently selectable primary selector.
- Every executable run needs a canonical pre-execution request receipt, one canonical execution receipt per selected case, and a canonical post-execution completion receipt; dependent preflight must receive controller-approved request and completion digests instead of trusting report-owned filters or outcomes.
- Promotable CQ evidence requires the documented controlled Linux host: invoke Cargo from `/` with absolute manifests and private configuration, reconstruct the config-free Git view index from `HEAD`, keep qualification artifacts, fixture side outputs, and support cleanup descriptor-owned, and do not run while another same-UID process can transiently mutate and restore the live checkout or linked Git and toolchain support state.
- Every selected exported Rust API item from the deterministic rustdoc inventory needs an exact case or parent-contract mapping; module-level tests and documentation alone do not close it.
- A whole upstream file, a broad Cargo filter, an all-green workspace suite, or a nearby test is supporting evidence, not a primary selector.
- Exact upstream provenance must name the path, complete test or source symbol, subcase, and gate marker where relevant.
- Exact bytes are required for contractual CLI and file-format output; semantic normalization must never erase a contractual difference.
- Structural comparators need adversarial tests proving that they reject missing, extra, wrong-weight, wrong-sign, wrong-target, and contractually reordered results.
- State-equivalence tests need separating states or tableaus and cannot rely only on gate-plus-inverse cancellation.
- Probabilistic cases need source-owned seeds, shots, buckets, expected probabilities or a declared two-sample model, effect-size targets, exact acceptance rules, and familywise error budgets.
- Statistical tests must use shared canonical integer boundaries and must never rerun until a favorable result appears.
- Every public input boundary needs positive, malformed, unsupported, overflow, path, width or count, and resource evidence where applicable.
- Streaming claims need bounded-buffer, early visitor or writer failure, and large-record evidence.
- Materialized and bounded-search APIs need explicit cap acceptance and first-rejection evidence.
- A selected correctness failure blocks the dependent benchmark and the final correctness qualification.

## Performance Rules

- Every selected feature needs a performance disposition, even when it is covered by a parent workload or is not performance relevant.
- A Stim ratio requires equivalent semantic work, identical fixture provenance, equal output obligations, a correctness preflight, and a faithful runner.
- Public CLI parity uses process-versus-process rows; in-process Stab CLI-body rows remain diagnostic.
- Use existing pinned `stim_perf` filters first, a symmetric public CLI second, and an ops-owned pinned-Stim adapter only when neither exposes the required phase.
- The adapter is benchmark infrastructure and must not create a Stab C++ compatibility surface.
- Promotable full evidence must pass the source-owned host policy for affinity, load, available memory, swap activity, and any required platform probes; an environment-unverified probe cannot become a source-owned performance claim.
- Run full performance qualification and architecture-scoped 1.25x conclusions separately on controlled Linux x86-64 and Linux AArch64 hosts; never combine ratios across architectures or promote emulated timing evidence.
- Promotable core comparisons must invoke the Stim adapter and bounded Stab qualification worker symmetrically, including setup-baseline and peak-memory evidence where memory is compared.
- Executable performance groups must be source-owned by `benchmarks/qualification-runtime-groups.json`, including immutable claim class, baseline eligibility, workload and measurement IDs, and exact CQ case IDs. The source-owned baseline must contain one matching disposition for every runtime group, must give diagnostic groups zero threshold rules, and must give threshold-eligible groups an exact complete rule set.
- Future promotable groups must obtain CQ case IDs from the runtime group contract and require controller-approved CQ request and completion digests. Offline report validation must reopen the CQ artifacts and reconstruct the evidence; caller-selected cases or self-described artifact digests are not acceptable.
- Split parse, compile, reference construction, execute, convert, serialize, search, transform, startup, and end-to-end phases whenever users can reuse an earlier phase.
- Pair exact named submeasurements and reject stale or missing ids.
- Never aggregate unlike phases into a row median or claim a ratio from a proxy that performs different work.
- Timed output must be consumed, work counters must be positive and equal, and untimed output digests must match before a ratio is computed.
- Full qualification uses calibrated batches, three warmups, nine interleaved paired samples, raw-sample retention, median paired ratios, relative median absolute deviation, and a fixed-seed bootstrap 95 percent confidence interval.
- PQ1 targets 350-millisecond calibration batches and independently rejects a common retained batch below the contractual 250-millisecond floor or above the 2-second ceiling; do not retry only a below-floor validation without fixing or recording the calibration policy.
- A primary row passes 1.25x only when both its median paired ratio and upper confidence bound are at most `1.25`.
- A slow comparable row cannot be waived.
- Noise classification must use paired-ratio relative MAD, not separate implementation-rate MAD. An initial noisy row receives exactly one complete group rerun with fresh warmups and the full sample count; retain both attempts and make the second authoritative regardless of outcome. Never rerun a non-noisy result or continue until favorable.
- A no-ratio disposition is allowed only when the validator proves that pinned Stim has no faithful comparator at the claimed surface and the reason names the condition that would retire it.
- Memory instrumentation cannot supply timing evidence.
- Process RSS comparisons, Stab allocation regressions, and scaling classifications must remain separate claims.
- Existing M12 thresholds remain active until replacement evidence is at least as strong and the migration is explicit.

## Suite Completion Versus Product Performance

The correctness suite is complete only when every selected executable case passes and every non-executable upstream case has a valid disposition.

The performance suite can be complete while reporting Stab rows slower than 1.25x Stim.
Such rows are successful measurements but failed performance targets and must remain red in the report with an owner, profiler evidence, and next action.
Do not describe Stab as having comprehensive performance parity until every selected comparable target passes.

The final report must state four conclusions separately:

1. Correctness inventory completeness and pass status.
2. Performance inventory completeness and comparator fidelity.
3. Comparable timing rows passing and failing the 1.25x target.
4. Memory and scaling regression status.

## Evidence Hygiene

- Source-owned completion evidence must identify a committed Stab revision with `local_modifications=false`.
- Generated local reports under `target/` are probes until a reviewed source-owned report references them.
- Every report must identify the exact Stim commit, fixture and inventory digests, toolchains, host, selection, seeds, sample counts, and command.
- Keep raw timing samples and paired order in generated artifacts.
- Never cite an old report as current after runners, fixtures, thresholds, output obligations, or hot paths change.
- Do not infer bounded memory from semantic success or infer semantic equivalence from matching throughput.
- Waiver files must reject stale and unused entries.
- Progress reports must preserve failed, noisy, and deferred counts instead of reporting only passes.
- Avoid turning large source files or feature checklists into evidence dumps; put executable state in manifests and concise conclusions in reports.

## Operational Constraints

- Do not add shell scripts.
- Keep `just` recipes thin and namespaced.
- Put selection, validation, subprocess, adapter build, timeout, statistics, report, and path logic in Rust ops binaries.
- Treat fixtures, output paths, adapter output, child stderr, generated reports, and repository contents as hostile inputs.
- Keep direct `std::simd` use isolated behind tested bit-kernel modules and retain scalar references.
- Refactor before adding unrelated behavior to a source file that is already near or above 1200 lines.

## Required Milestone Evidence

Each CQ progress report must include:

- Planned, executable, passed, failed, deferred-product, semantic-mining, not-applicable, and superseded counts.
- Counts by domain and comparator.
- Exact selectors for every new primary case.
- Statistical budget and resource-boundary results where applicable.
- Audit and full-review findings and their resolution.

Each PQ progress report must include:

- Measured, covered-by-parent, not-performance-relevant, and no-faithful-comparator counts.
- Retained, reworked, diagnostic, superseded, and removed inherited benchmark rows.
- Raw measurement and exact pair counts.
- Passed, failed, noisy, and report-only 1.25x outcomes.
- Memory and scaling outcomes.
- Every slow or noisy row's owner, profiler-note path, and next action.
- Exact clean-revision and host metadata for source-owned claims.

## Final Verification

Run targeted commands from each milestone throughout implementation.
After all planned commands exist, run the final program checks:

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings
cargo test --workspace --quiet
just oracle::version
just oracle::matrix --check
just oracle::blockers --check-selectors
just oracle::run --implemented-only
just qualification::correctness-check
just qualification::correctness-run --tier pr
just qualification::correctness-run --tier full
just qualification::correctness-run --tier soak
just bench::smoke
just bench::qualification-check
just bench::qualification-probe --group pq1-process-contract-smoke
just bench::qualification-probe --group pq1-adapter-protocol-smoke
just bench::qualification-run --tier pr --out target/benchmarks/qualification/pq1-pr
just bench::qualification-run --tier full --out target/benchmarks/qualification/pq1-full
just bench::qualification-run --tier soak --out target/benchmarks/qualification/pq1-soak
just bench::qualification-report --input target/benchmarks/qualification/pq1-pr
just bench::qualification-report --input target/benchmarks/qualification/pq1-full
just bench::qualification-report --input target/benchmarks/qualification/pq1-soak
just bench::qualification-regression --input target/benchmarks/qualification/pq1-pr
just bench::qualification-regression --input target/benchmarks/qualification/pq1-full
just bench::qualification-regression --input target/benchmarks/qualification/pq1-soak
just bench::primary-beta --baseline <fresh-primary-baseline>
just bench::primary-regression --baseline <fresh-primary-baseline> --report target/benchmarks/qualification/m12-regression
just bench::primary-memory-regression --baseline <fresh-primary-baseline>
just maintenance::pre-commit
```

The CQ1 correctness commands are mandatory and complete. The implemented PQ1 qualification commands are mandatory for PQ1 acceptance; later product qualification commands become mandatory as their workload groups land.
Do not fake an early pass by omitting commands that the active milestone is required to implement.

## Completion Criteria

This goal is complete only when:

- CQ0 through CQ6 satisfy every acceptance criterion.
- PQ0 through PQ7 satisfy every acceptance criterion.
- Every implemented selected feature has a validated correctness case disposition and performance disposition.
- Every selected executable correctness case passes.
- Every benchmark ratio references passing correctness cases and equivalent semantic work.
- Every measured performance group has a faithful comparison, an explicit failed target, or a validated no-faithful-comparator disposition.
- Every streaming, compact, materialized, and search resource claim has machine-checked evidence.
- Existing M12 timing and memory gates remain active or have been explicitly superseded by equal or stronger coverage.
- The final comprehensive report separates suite completeness from timing parity and lists all optimization failures.
- Milestone audits and full-code-review have no unresolved confirmed finding.
- Documentation, manifests, schemas, reports, and operational commands agree.
- Final verification passes from the resulting worktree.

## Explicit Deferrals

Python bindings and Python object shape, JS/WASM, diagrams, `repl`, QASM, Quirk, Crumble, ecosystem packages, GPU, exact random-stream parity, public graph or vector simulator products, C++ header compatibility, full ErrorMatcher provenance, `explain_errors`, deprecated `--detector_hypergraph`, and behavior outside the selected implemented Rust and CLI contracts remain future work.
They do not block this qualification goal and must not be counted as passes.
