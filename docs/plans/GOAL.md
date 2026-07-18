# Goal: Qualify Stab Correctness And Performance Against Stim

## Status

Active execution goal as of 2026-07-17.

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
- PQ2 gate, bit-kernel, and Pauli evidence: `docs/plans/pq2-gate-name-hash-qualification-progress-report.md`, `docs/plans/pq2-simd-word-popcount-qualification-progress-report.md`, `docs/plans/pq2-simd-bits-xor-qualification-progress-report.md`, `docs/plans/pq2-simd-bits-not-zero-qualification-progress-report.md`, `docs/plans/pq2-sparse-xor-qualification-progress-report.md`, `docs/plans/pq2-bit-matrix-transpose-qualification-progress-report.md`, `docs/plans/pq2-pauli-string-multiplication-qualification-progress-report.md`, and `docs/plans/pq2-pauli-string-iterator-qualification-progress-report.md`.
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

Program checkpoint: CQ0 is source-current at correctness inventory digest `4c940e983df10a7c95cc512939f4a0cce79f1865e141739af9378db581ea5f87`, with 2,886 upstream records, 1,972 default-feature public Rust API items, and 1,744 evidence parents. Of those parents, 580 are implemented, 17 are evidence-close, and 1,147 remain planned for later qualification. Core revision `8503f458eac09fe94dba54cf8d6e16e88e195df4` strengthens the dedicated Pauli iterator owner with an independent sparse enumerator, exact pinned scales, word boundaries, borrowed-storage reuse, restart, and public resource limits. Clean pre-migration revision `f2388dccc01abb7ef89e5f56d9062c6656837470` published and replayed the exact three-case correctness prerequisite, reproducible workers, both complete adapter probes, 12 first-attempt passing reports and regressions, four rollups, and two completion receipts under performance inventory `ad3b6640e04855ac76d4851f856bb405e7c80c55dbcd67b204d67ea41d40c1eb`. Those receipts authorized retirement of only the bundled `m6-pauli-iter` timing threshold and its two exact mappings. PQ0 is source-current at performance inventory digest `868eb831a034042b43573fed612af14db225421a2733bbf10e4a5eb2b515ec90`: fifteen PQ2 product contracts remain executable with 21 independent `1.25x` threshold pairs, the inherited iterator row is superseded, and its memory baseline remains guarded for PQ6. This digest refresh changes only the synchronized CQ2 evidence-status anchor in `docs/stab-feature-checklist.md`; it does not alter a runtime group, threshold, workload, scale, comparator, or correctness binding. Clean reviewed iterator evidence revision `afaf0bf7f236b9f6ae6f72c19bbbdfea94d26632` remains accepted historical AArch64 evidence under recorded performance inventory `48eacf03a2ecdca917c05aade52b7e17c9ead1be8b75b203e1d43c2f3b3b7dbf`, with all-output correctness at the accepted maximum, 12 first-attempt passing reports and regressions, four rollups, two completion receipts, completed milestone audit, and two clean final GPT-5.6/max review lanes. Clean revision `cd1e33e10f45995ccaca498547ff5aa88bfe51bb` remains accepted historical Pauli multiplication evidence under its recorded inventories, and clean revision `f912cc3af1f13cc9fab798d69937c155d37d83a0` remains accepted historical transpose evidence under its recorded inventory. Earlier `not_zero`, sparse-XOR, and dense-XOR outcomes likewise remain historical under their exact identities. No prior correctness or performance report is relabeled as current after its bound source contract changes. Setup and peak RSS remain report-only observations until PQ6 defines a machine-checked growth rule. No threshold, comparator-fidelity, equal-work, output-identity, or migration rule may be relaxed.
CQ2 implementation and ownership are complete for its eight selected domains and 271 implemented exact parents: `.stim` 29, `.dem` 28, result formats 39, gate contract 60, bit kernels 12, circuit API 24, Generation 25, and Algebra 54. Clean correctness revision `6351fe0a529462efd9f18a88bd59c08cfec9f81b` passed the PR tier's 145 selected parents and all 271 parents in both full and soak under digest `4c940e983df10a7c95cc512939f4a0cce79f1865e141739af9378db581ea5f87`; those reports and exact preflights are a complete historical checkpoint with `local_modifications=false`. Owner-cap hardening in `763b04055f969668a4152e019ed62a3555cd0476` changed the qualification controller, and current offline replay now rejects the older request receipts as stale by design. Commit the synchronized controller, inventory, and documentation state, then rerun PR, full, and soak from that exact clean revision before citing source-current all-domain execution or consuming CQ2 preflight in new performance evidence.
The active execution checkpoint is the source-current CQ2 evidence refresh. After it passes and is documented, resume PQ2 by applying the paired performance harness to the completed CQ2 deterministic surfaces, replacing inherited proxy or heterogeneous rows with equivalent-work phase-specific groups, binding each runtime group to exact passing CQ2 prerequisites and output digests, adding the required small, medium, and large scale families, and keeping failed or noisy 1.25x outcomes visible. Do not promote any ratio until its exact current-digest correctness preflight, comparator, work count, and output obligations validate.
The first five proving groups passed on the controlled Linux AArch64 host under preceding performance inventories and remain historical after later inventory or shared-worker changes. Clean revision `5d226c94ece70f96d0b771f9c8cde7464ccd261b` closes the fifth group's historical AArch64 evidence chain without weakening the `1.25x` gate. Both under-specifications revealed by that audit are now resolved in `docs/plans/milestone-spec-gaps.md`: Stab allocation instrumentation covers every dense-XOR scale plus the accepted maximum, and clean implementation revision `b208a359f3f7676e2b07d64a5dc8caca208abf6a` adds completion receipt schema version 1 for every later executable slice. The sixth `not_zero`, seventh sparse-XOR, eighth BitMatrix transpose, ninth Pauli multiplication, and tenth split Pauli iterator slices are complete on controlled Linux AArch64 at their recorded inventories; their progress reports record exact correctness, worker, report, regression, rollup, completion, audit, and review evidence. The latest complete 271-parent CQ2 checkpoint is historical at clean revision `6351fe0a529462efd9f18a88bd59c08cfec9f81b`; replace it with a source-current clean chain before starting promotable eleventh-slice timing. Native x86-64 execution remains an unclaimed evidence target, and x86 adapter builds must prove that they inherit Stim's resolved machine flags before producing any ratio.
Keep PQ1's `pq1-adapter-protocol-smoke` ratio permanently diagnostic and never report it as product speed evidence.
Do not reopen CQ0 or PQ0 inventory semantics unless pinned-source drift, a newly exported default-feature API, a stale referenced id, a changed checklist or benchmark source of truth, or a confirmed inventory defect changes a frozen digest.
Do not treat PQ0's 10 retained inherited rows as qualified evidence: the current inventory reports 158 missing correctness preflights, 158 missing output digests, 58 asymmetric CLI rows, 73 missing comparators, 123 missing scale families, and 20 heterogeneous selections. The fifteen implemented product contracts replace only their exact parse, canonical-serialization, all-gate-name-hash, toggle-plus-popcount, complete-vector dense-XOR, three position-specific `not_zero`, sparse row-XOR, sparse item-toggle, allocating transpose, square in-place transpose, non-identity Pauli right multiplication, X/Z weight-range Pauli iteration, and X/Y/Z singleton Pauli iteration groups.

## Current Execution Contract

First commit the synchronized post-hardening controller, inventories, and documentation. From that exact clean revision, run the all-domain CQ2 PR, full, and soak selections once, replay every report, validate one exact controller-bound preflight per tier, and record the revision, inventory, request, report, completion, and preflight digests. Any source change affecting the runner or inventory makes that refresh historical and requires another clean chain.
The sixth through tenth executable slices are closed on controlled Linux AArch64 and must not be reopened merely to produce a newer aggregate digest.
The eighth slice remains two independent contracts from API ownership through completion: public allocating transpose and public square in-place transpose. Do not aggregate their timing or allocation outcomes, substitute `transpose_into`, weaken the exact fixture or output obligations, or retire the retained M12 memory baseline before PQ6 supplies equal or stronger evidence.
Clean revision `f912cc3af1f13cc9fab798d69937c155d37d83a0` is the accepted reviewed transpose evidence revision. Its two-case correctness preflight, reproducible workers, both probes, 12 first-attempt reports and regressions, four rollups, two completion receipts, exact ratios, artifact hashes, and review-fix history are recorded in `docs/plans/pq2-bit-matrix-transpose-qualification-progress-report.md`.
The ninth PQ2 slice, `PERFQ-M6-PAULI-STRING`, is closed on controlled Linux AArch64 at clean evidence revision `cd1e33e10f45995ccaca498547ff5aa88bfe51bb`. It qualifies only equal-width public `PauliString::right_multiply_in_place_returning_log_i_scalar` against pinned Stim with deterministic non-identity operands at 10,000, 100,000, and 1,000,000 qubits. Its exact two-case correctness preflight, reproducible workers, probe, six first-attempt reports and regressions, two rollups, completion receipt, allocation checks, migration history, audit fixes, and review fixes are recorded in `docs/plans/pq2-pauli-string-multiplication-qualification-progress-report.md`.
Do not use the inherited identity-only `m6-pauli-string` ratio as evidence: Stab's public identity-right fast path makes that legacy comparison O(1) on the Stab side. Migration commit `42c132f2c49538364649cd90962166223c72b4c6` retired only its row-level and three exact pair-level timing thresholds and scale mappings. Preserve the memory baseline until PQ6 supplies equal or stronger memory evidence.
The accepted Pauli chain preserves the direct public lifecycle, returned phase, odd/even state, right-operand immutability, zero timed Stab allocations, the 1,048,576-qubit accepted maximum, first rejection, pre-setup semantic-work overflow rejection, exact output digests, and independent thresholds at every scale. No portable SIMD was needed because the faithful scalar path passed every gate comfortably; any later SIMD work must remain behind the tested private bit-kernel boundary and restart affected evidence.
The tenth PQ2 slice is the split Pauli-string iterator contract in `docs/plans/comprehensive-stim-performance-qualification-plan.md`: `PERFQ-M6-PAULI-ITER` owns X/Z weights 2 through 5, and `PERFQ-M6-PAULI-ITER-SINGLETON` owns X/Y/Z weight 1. Clean evidence revision `afaf0bf7f236b9f6ae6f72c19bbbdfea94d26632` closes the machine-checkable evidence for both groups while retaining separate correctness summaries, scale families, timing outcomes, allocation contracts, rollups, and completion receipts for the exact upstream constructor-plus-borrowed-traversal lifecycle.
Clean pre-migration revision `f2388dccc01abb7ef89e5f56d9062c6656837470` authorized the narrow iterator timing migration, and migration commit `d706634eeaa536b2ce48d3dc9431b4feb513317f` retired only the bundled timing threshold and two exact small-scale mappings. The accepted historical post-migration chain at inventory `48eacf03a2ecdca917c05aade52b7e17c9ead1be8b75b203e1d43c2f3b3b7dbf` has 12 first-attempt passing reports and regressions, four replayed rollups, two replayed completion receipts, and median ratios from `0.025664x` to `0.568566x` with worst upper bound `0.570628x`. No ratio is promoted under current inventory `868eb831a034042b43573fed612af14db225421a2733bbf10e4a5eb2b515ec90` until its exact group is rerun from one clean revision. `benchmarks/m12-primary-memory-baseline.json` remains explicitly guarded for PQ6.
The iterator chain freezes exact hostile-boundary and overflow receipts, independently reproduced sequence digests, complete yielded X and Z planes for every singleton output through the 1,048,576-qubit accepted maximum, private Stab build receipt schema version 3, adapter receipt and contract-preflight schema version 10 with 140 receipts, and qualification report schema version 28. Do not use the retired legacy row ratio, pre-migration reports as post-migration evidence, or diagnostic probe timing as product evidence. Exact hashes, ratios, resource outcomes, migration history, audit fixes, review fixes, and remaining scope are recorded in `docs/plans/pq2-pauli-string-iterator-qualification-progress-report.md`.
The active eleventh slice is the two-contract Clifford-string plan in `docs/plans/comprehensive-stim-performance-qualification-plan.md`. Existing `PERFQ-M6-CLIFFORD-STRING` owns only the exact pinned identity workload, while new `PERFQ-M6-CLIFFORD-STRING-NON-IDENTITY` must cover equal-width public in-place multiplication over a deterministic complete 24-by-23 non-identity composition cycle. Do not let Stab's O(1) identity fast path substitute for the non-identity measurement, and do not begin formal timing until exact correctness, independent output, allocation, hostile-boundary, and worker-receipt contracts land.
Do not infer completion of stabilizer Algebra, deterministic PQ2, or the comprehensive suite from the completed iterator groups or the selected Clifford slice.
Keep native Linux x86-64 execution and PQ6 memory-growth qualification visible as separate later obligations.

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
just bench::qualification-probe --group pq2-sparse-xor-row-adapter-smoke --iterations 2 --work-items 1997
just bench::qualification-probe --group pq2-sparse-xor-item-adapter-smoke --iterations 2 --work-items 7
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
