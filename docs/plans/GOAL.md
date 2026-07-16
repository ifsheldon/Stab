# Goal: Qualify Stab Correctness And Performance Against Stim

## Status

Active execution goal as of 2026-07-16.

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
- Completed CQ2 execution evidence: `docs/plans/cq2-deterministic-qualification-progress-report.md`.
- Completed CQ1 harness evidence: `docs/plans/cq1-correctness-harness-progress-report.md`.
- Performance execution and acceptance: `docs/plans/comprehensive-stim-performance-qualification-plan.md`.
- Completed PQ1 harness evidence: `docs/plans/pq1-performance-harness-progress-report.md`.
- Historical passing reports for the first two PQ2 product groups: `docs/plans/pq2-circuit-parse-qualification-progress-report.md` and `docs/plans/pq2-circuit-canonical-print-qualification-progress-report.md`.
- PQ2 gate and bit-kernel evidence: `docs/plans/pq2-gate-name-hash-qualification-progress-report.md`, `docs/plans/pq2-simd-word-popcount-qualification-progress-report.md`, `docs/plans/pq2-simd-bits-xor-qualification-progress-report.md`, and `docs/plans/pq2-simd-bits-not-zero-qualification-progress-report.md`.
- Completed PQ2 completion-receipt infrastructure evidence: `docs/plans/pq2-completion-receipt-progress-report.md`.
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

Program checkpoint: CQ0 is source-current at correctness inventory digest `ccb80eb4b660a375b59460c3b7fa03a810abd6f868735b566735378105db22b2`, with 2,886 upstream records, 1,972 default-feature public Rust API items, and 1,744 evidence parents. Of those parents, 580 are implemented, 17 are evidence-close, and 1,147 remain planned for later qualification. PQ0 and eight PQ2 product contracts are source-current at performance inventory digest `0161ab09015487ee2a1298be8dafe7c744b426b28a4e9fbdbd688e775c1655a0`: the first five historical groups plus independent early-hit, all-zero, and late-hit `not_zero` groups. The current `not_zero` contracts force an optimizer-opaque immutable input reference on both workers so calibration measures every requested scan instead of a precomputed Boolean. The timing policy records the 2-second independent ceiling and bounded 20-second wide-ratio common ceiling without changing equal-work pairing or the `1.25x` gate. Clean revision `60b732c77f1828058fbd65ec6c5c4ad582467fd1` supplies 18 passing first-attempt full and soak AArch64 measurements at the source-current inventory, 18 successful report and regression replays, six replayed rollups, and three replayed completion receipts. Median ratios range from `0.032329x` to `0.663712x`, and every bootstrap confidence-interval upper bound is below `0.665x`. Allocation instrumentation covers every pattern at every runtime scale and the accepted maximum. Final milestone re-audit and independent GPT-5.6/max review report no remaining finding or new under-specification. The source-current inventory retains only the still-active XOR replacement mapping and threshold pair. Clean revision `5d226c94ece70f96d0b771f9c8cde7464ccd261b` supplies historical dense-XOR evidence at performance inventory `fb50789c58786219c807c79e87202396b17563ee7cb584bcda4d3379007ed716`, with median ratios from `0.374633x` to `0.559828x` and worst confidence-interval upper bound `0.561257x`. Setup and peak RSS remain report-only observations until PQ6 defines a machine-checked growth rule. No threshold or comparator fidelity rule may be relaxed.
CQ2 implementation and ownership remain complete for its eight selected domains and 271 implemented exact parents: `.stim` 29, `.dem` 28, result formats 39, gate contract 60, bit kernels 12, circuit API 24, Generation 25, and Algebra 54. The exact gate-hash prerequisite passed from clean revision `c76b7071fc4d7ac358ef3a2fffc053ea675bd05f` under the current correctness digest, but its dependent timing reports remain historical under an earlier performance inventory and worker identity. The clean all-domain PR, full, and soak reports from revision `bae9e01cb3fedaf9d37958e6827b064c635b9898` and focused parser/printer report from revision `ba70a52025fdd4122ac97cec263725b2ec56e431` remain historical after the hash owner changed the digest. Rerun the full 271-case CQ2 family before citing all-domain execution as current-digest evidence.
The active milestone is PQ2. Apply the paired performance harness to the completed CQ2 deterministic surfaces, replace inherited proxy or heterogeneous rows with equivalent-work phase-specific groups, bind each runtime group to exact passing CQ2 prerequisites and output digests, add the required small, medium, and large scale families, and keep failed or noisy 1.25x outcomes visible. Do not promote any ratio until its exact current-digest correctness preflight, comparator, work count, and output obligations validate.
The first five proving groups passed on the controlled Linux AArch64 host under preceding performance inventories and remain historical after later inventory or shared-worker changes. Clean revision `5d226c94ece70f96d0b771f9c8cde7464ccd261b` closes the fifth group's historical AArch64 evidence chain without weakening the `1.25x` gate. Both under-specifications revealed by that audit are now resolved in `docs/plans/milestone-spec-gaps.md`: Stab allocation instrumentation covers every dense-XOR scale plus the accepted maximum, and clean implementation revision `b208a359f3f7676e2b07d64a5dc8caca208abf6a` adds completion receipt schema version 1 for every later executable slice. The sixth `not_zero` slice is complete on controlled Linux AArch64 at the source-current inventory; `docs/plans/pq2-simd-bits-not-zero-qualification-progress-report.md` records the exact correctness, worker, report, regression, rollup, completion, audit, and review evidence. Rerun the complete 271-parent CQ2 family at the next simultaneous-current program checkpoint instead of relabeling historical reports. Native x86-64 execution remains an unclaimed evidence target, and x86 adapter builds must prove that they inherit Stim's resolved machine flags before producing any ratio.
Keep PQ1's `pq1-adapter-protocol-smoke` ratio permanently diagnostic and never report it as product speed evidence.
Do not reopen CQ0 or PQ0 inventory semantics unless pinned-source drift, a newly exported default-feature API, a stale referenced id, a changed checklist or benchmark source of truth, or a confirmed inventory defect changes a frozen digest.
Do not treat PQ0's 13 retained rows as qualified evidence: the current inventory reports 158 missing correctness preflights, 158 missing output digests, 58 asymmetric CLI rows, 73 missing comparators, 123 missing scale families, and 20 heterogeneous selections. The eight implemented contracts replace only their exact parse, canonical-serialization, all-gate-name-hash, toggle-plus-popcount, complete-vector dense-XOR, and three position-specific `not_zero` groups.

## Sixth-Slice Closure Contract

1. Keep the three `not_zero` groups independent through inventory, runtime selection, adapter probe, report, regression, rollup, completion receipt, audit, and review. Never use an early result to close all-zero or late-hit work.
2. Commit the implementation and contract documentation before asking the sealed builders to materialize it. A diagnostic probe from a dirty checkout is not clean evidence and an untracked comparator file must remain absent from the materialized commit.
3. From the clean implementation revision, run `just qualification::correctness-run --tier full --case cq-evidence-qualification-b1530dc4e48e942d --case cq-evidence-qualification-ba252d42660a41ce` and the corresponding correctness report and preflight commands required by each performance run. Do not reuse stale correctness producer commits.
4. Run `just bench::qualification-worker-reproducibility` and all three source-owned adapter probes. Require exact 10,000-bit input and output receipts and retain the full 42-receipt preflight identity.
5. For each group and each `small`, `medium`, and `large` scale, publish one clean `full` report and one clean `soak` report. Replay each report immediately and run source-owned regression against every report. A failed or noisy report remains visible and gains a profiler note; do not repeat it in search of a pass.
6. Produce and replay separate full and soak scale-family rollups for each group. Then publish and replay one schema-version-1 completion receipt per group, bound to the exact source reports, regressions, rollups, worker reproducibility, adapter probe, clean commit, and CPU identity.
7. Run the milestone-audit skill against the sixth-slice objective, all three contracts, tests, rejection and allocation boundaries, reports, rollups, completion receipts, and legacy-threshold migration. Fix implementation defects. Log only genuine newly exposed under-specification in `docs/plans/milestone-spec-gaps.md`.
8. Run an independent GPT-5.6/max full-code-review subagent over Rust, C++, SIMD, inventory generation, hostile-input bounds, schemas, tests, benchmark fidelity, evidence publication, and documentation. Fix every confirmed finding and rerun affected evidence.
9. Write `docs/plans/pq2-simd-bits-not-zero-qualification-progress-report.md` with exact commands, clean revision, host and CPU identity, all 18 report outcomes, nine scale ratios per tier, worst confidence bound per group, replay and completion paths, audit and review closure, legacy-threshold disposition, and remaining x86-64 scope.
10. Retire or alter the legacy M12 `not_zero` threshold only after the early group's clean completion receipt, migration review, and current docs all agree. The all-zero and late groups never substitute for that exact mapping.

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
- Executable performance groups must be source-owned by `benchmarks/qualification-runtime-groups.json`, including immutable claim class, baseline eligibility, workload and measurement IDs, named scales with positive work counts, and exact CQ case IDs. Runtime commands must select these group and scale ids and must not accept caller-defined replacement work counts. Reports must bind the selected scale and work count exactly. The source-owned baseline must contain one matching disposition for every runtime group, must give diagnostic groups zero threshold rules, and must give threshold-eligible groups an exact complete rule set.
- Product PR reports may be retained as nonpromotable diagnostic evidence after exact CQ preflight. Promotion is derived rather than caller-selected: only full or soak evidence from an unchanged clean revision, a verified host, and passed exact correctness prerequisites can enter regression dispatch.
- Every executable PQ2 product slice whose closure is claimed after completion receipt schema version 1 was introduced must publish one completion receipt from its clean source revision after worker reproducibility, the exact adapter probe, idempotent full and soak report replays, passing regression at every scale, and passing full and soak rollup replays with one exact CPU identity. `qualification-completion-report` must rerun those handlers and reconstruct the receipt and preflight byte for byte. Historical slices accepted under the preceding contract remain explicitly historical and must not be relabeled as receipt-backed. Record milestone audit and GPT-5.6/max review separately because human review is not a machine-self-certifying operation.
- Future promotable groups must obtain CQ case IDs from the runtime group contract and require controller-approved CQ request and completion digests. Offline report validation must reopen the CQ artifacts and reconstruct the evidence; caller-selected cases or self-described artifact digests are not acceptable.
- Split parse, compile, reference construction, execute, convert, serialize, search, transform, startup, and end-to-end phases whenever users can reuse an earlier phase.
- Pair exact named submeasurements and reject stale or missing ids.
- Never aggregate unlike phases into a row median or claim a ratio from a proxy that performs different work.
- Timed output must be consumed, work counters must be positive and equal, and untimed output digests must match before a ratio is computed.
- A Stab zero-allocation timed-body claim requires allocator instrumentation at every runtime scale and the accepted maximum. Pinned Stim source inspection proves comparator shape only, not a Stim allocation count; cross-implementation allocation claims require instrumentation on both implementations, and process RSS remains separate PQ6 evidence.
- Full qualification uses calibrated batches, three warmups, nine interleaved paired samples, raw-sample retention, median paired ratios, relative median absolute deviation, and a fixed-seed bootstrap 95 percent confidence interval.
- PQ1 independently calibrates each implementation to a 350-millisecond target and a 2-second ceiling. A common equal-work batch normally requires both implementations between 250 milliseconds and 2 seconds. Wide-ratio mode may permit only the implementation that selected fewer independent iterations to exceed 2 seconds, while the common-iteration owner remains at or below 2 seconds and both remain between 250 milliseconds and the hard 20-second common ceiling. Offline replay must derive this mode from raw decisions and receipts; callers and reports cannot select it.
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
just bench::qualification-probe --group pq2-circuit-parse-adapter-smoke --iterations 2 --work-items 64
just bench::qualification-probe --group pq2-circuit-canonical-print-adapter-smoke --iterations 2 --work-items 64
just bench::qualification-probe --group pq2-gate-name-hash-adapter-smoke --iterations 4 --work-items 5248
just bench::qualification-probe --group pq2-simd-word-popcount-adapter-smoke --iterations 2 --work-items 262144
just bench::qualification-probe --group pq2-simd-bits-xor-adapter-smoke --iterations 2 --work-items 262144
just bench::qualification-probe --group pq2-simd-bits-not-zero-early-adapter-smoke --iterations 2 --work-items 10000
just bench::qualification-probe --group pq2-simd-bits-not-zero-all-zero-adapter-smoke --iterations 2 --work-items 10000
just bench::qualification-probe --group pq2-simd-bits-not-zero-late-adapter-smoke --iterations 2 --work-items 10000
just bench::qualification-worker-reproducibility
just bench::qualification-run --tier pr --out target/benchmarks/qualification/pq1-pr
just bench::qualification-run --tier full --out target/benchmarks/qualification/pq1-full
just bench::qualification-run --tier soak --out target/benchmarks/qualification/pq1-soak
just bench::qualification-report --input target/benchmarks/qualification/pq1-pr
just bench::qualification-report --input target/benchmarks/qualification/pq1-full
just bench::qualification-report --input target/benchmarks/qualification/pq1-soak
just bench::qualification-regression --input target/benchmarks/qualification/pq1-pr
just bench::qualification-regression --input target/benchmarks/qualification/pq1-full
just bench::qualification-regression --input target/benchmarks/qualification/pq1-soak
just bench::qualification-rollup-report --input <scale-family-rollup>
just bench::qualification-completion --group <runtime-group> --full-input <full-scale-report> ... --soak-input <soak-scale-report> ... --full-rollup <full-rollup> --soak-rollup <soak-rollup> --out <completion-directory>
just bench::qualification-completion-report --input <completion-directory>
just bench::primary-beta --baseline <fresh-primary-baseline>
just bench::primary-regression --baseline <fresh-primary-baseline> --report target/benchmarks/qualification/m12-regression
just bench::primary-memory-regression --baseline <fresh-primary-baseline>
just maintenance::pre-commit
```

The CQ1 correctness commands are mandatory and complete. The implemented PQ1 qualification commands are mandatory for PQ1 acceptance; later product qualification commands, including completion receipt publication and replay for every executable PQ2 slice, become mandatory as their workload groups land.
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
