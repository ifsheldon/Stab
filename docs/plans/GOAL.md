# Goal: Qualify Stab Correctness And Performance Against Stim

## Status

Active execution goal as of 2026-07-22.

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
- CQ2 implementation status and current-versus-historical execution evidence: `docs/plans/cq2-deterministic-qualification-progress-report.md`.
- Completed CQ1 harness evidence: `docs/plans/cq1-correctness-harness-progress-report.md`.
- Performance execution and acceptance: `docs/plans/comprehensive-stim-performance-qualification-plan.md`.
- Completed PQ1 harness evidence: `docs/plans/pq1-performance-harness-progress-report.md`.
- Historical passing reports for the first two PQ2 product groups: `docs/plans/pq2-circuit-parse-qualification-progress-report.md` and `docs/plans/pq2-circuit-canonical-print-qualification-progress-report.md`.
- PQ2 gate, bit-kernel, Pauli, and Clifford evidence: `docs/plans/pq2-gate-name-hash-qualification-progress-report.md`, `docs/plans/pq2-simd-word-popcount-qualification-progress-report.md`, `docs/plans/pq2-simd-bits-xor-qualification-progress-report.md`, `docs/plans/pq2-simd-bits-not-zero-qualification-progress-report.md`, `docs/plans/pq2-sparse-xor-qualification-progress-report.md`, `docs/plans/pq2-bit-matrix-transpose-qualification-progress-report.md`, `docs/plans/pq2-pauli-string-multiplication-qualification-progress-report.md`, `docs/plans/pq2-pauli-string-iterator-qualification-progress-report.md`, and `docs/plans/pq2-clifford-string-qualification-progress-report.md`.
- Completed PQ2 completion-receipt infrastructure evidence: `docs/plans/pq2-completion-receipt-progress-report.md`.
- In-progress twelfth-slice evidence and retained attempts: `docs/plans/pq2-dem-parse-print-qualification-progress-report.md`.
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

Program checkpoint: CQ0 is source-current at correctness inventory digest `fbaa2bdf8bc0eea01b2aca385a4e537de11c3b35099671cca0e2775950c2fbb0`, with 2,886 upstream records, 1,974 default-feature public Rust API items, and 1,744 evidence parents. Of those parents, 580 are implemented, 17 are evidence-close, and 1,147 remain planned for later qualification. PQ0 is source-current at post-migration performance inventory digest `3f51801b592b0cb8dc3b340cced3dc3b7644b913168073c7d4106188e444d83d`: nineteen PQ2 product contracts remain executable, each with one exact `1.25x` rule over three scales, for 57 independently reported outcomes. Clean pre-migration revision `d9e2405d18cfff05d9b5d908525394476b0edcbc` passed and replayed both DEM adapter probes, worker reproducibility, the exact focused CQ prerequisite, all twelve first-attempt full and soak scale reports and regressions, four rollups, and both completion receipts under preceding performance digest `a98f57cf194f3a021d321266656cf688c9f7780fb39fa337475e8132411eb88a`. Parse medians range from `1.102449x` through `1.159075x`, print medians range from `0.559351x` through `0.580445x`, and the worst confidence-interval upper bound is `1.167830x`. Focused migration commit `1cfecd64cde4a5effdf07fdaabdbe51017e25a4a` retired only the exact legacy parse timing threshold and print no-ratio waiver, preserved both memory baselines, and marked both old rows superseded and non-primary. Schema-version-2 `benchmarks/qualification-threshold-migrations.json` binds both authorizations, their distinct guard kinds, the complete replacement scale families, migration revision and inventory, and retained memory evidence. The full post-migration machine chain has not run, so no pre-migration ratio may be relabeled as source-current timing. The harness remains at private Stab build-receipt schema version 6, adapter receipt schema version 12, contract-preflight schema version 13 with 228 probes, and qualification report schema version 32. Clean reviewed Clifford, iterator, Pauli multiplication, transpose, and earlier kernel evidence remains accepted historical evidence only under its recorded inventory. Setup and peak RSS remain report-only until PQ6 defines a machine-checked growth rule. No threshold, comparator fidelity, semantic-work unit, output identity, batching policy, publication-retention rule, or migration rule may be relaxed.
The current inventory removes six stale heterogeneous M12 row thresholds for stabilizer-to-tableau conversion, tableau construction, tableau iteration, CLI dispatch, frame simulation, and split one-shot sampling analysis. Their typed comparison contracts cannot produce defensible ratios for the current workloads, so they remain explicit planned report-only groups until faithful equal-work runners exist.
Clean revision `859bf202bdd4bdfbca07e9b1d647afb1b0542846` has a complete and accepted historical Clifford correctness and performance machine chain under correctness digest `648e7ea5a66997a810498dc871257bd654c7f9af9304651d43a88103eded0289` and performance digest `a47866ba5eab70392dd2754391d3d7d8588567a7cbfc1f81a569be813804ce51`. Revisions `29a29d5f68767e4ab131b051c88f6b77417e0338` and `0b86f07881198c57df1237b23a7d7c0084f2a272` are historical and review-rejected because later reviews found exact-case admission, early formal-session binding, all-write staging-abort, retained Stim-inventory, and dual session-error defects. Follow-up milestone audit found no implementation defect or newly revealed under-specification, independent full-code-review found no unresolved P0 through P3 issue, and final verification passed. The current DEM worker, adapter, fixture, schema, and inventory changes do not relabel that chain as current evidence.
CQ2 implementation and exact ownership remain complete for eight selected domains and 271 implemented parents: `.stim` 29, `.dem` 28, result formats 39, gate contract 60, bit kernels 12, circuit API 24, Generation 25, and Algebra 54. Clean correctness revision `3f2f382627c8421de0a668819d467a9f252de20f` passed PR, full, and soak under preceding digest `4c940e983df10a7c95cc512939f4a0cce79f1865e141739af9378db581ea5f87`; focused revisions `ac20ffca`, `91f62d0a78659da2e8e264a6968b3c6cd32456de`, and `da7c787d1e9f49110d7054868b146b5fb7d7bda4` passed their exact three Clifford prerequisites under their recorded preceding digests. All remain historical evidence for their exact producers and inventories. Revision `859bf202bdd4bdfbca07e9b1d647afb1b0542846` passed and replayed the then-current exact three-case focused Clifford replacement under correctness digest `648e7ea5a66997a810498dc871257bd654c7f9af9304651d43a88103eded0289`, with its request, report, completion, preflight, and Markdown digests recorded in `docs/plans/pq2-clifford-string-qualification-progress-report.md`. The source-current inventory is now `fbaa2bdf8bc0eea01b2aca385a4e537de11c3b35099671cca0e2775950c2fbb0` after the third source-line-only DEM parser refresh and has no current all-domain execution claim. Exact historical digests remain recorded in `docs/plans/cq2-deterministic-qualification-progress-report.md`.
The eleventh PQ2 Clifford-string slice is complete on controlled Linux AArch64. Revision `859bf202bdd4bdfbca07e9b1d647afb1b0542846` passed all 12 first-attempt full and soak scale reports, their replays and regressions, four rollups and replays, and two completion producers and independent replays. Identity median Stab-to-Stim seconds-per-work ratios range from `0.000179x` to `0.017874x`; non-identity median ratios range from `0.906236x` to `0.927767x`; and the worst confidence-interval upper bound is `0.928804x`, below the exact `1.25x` gate. Clean pre-migration revision `127d6661a9e00872fc4aa4c0b0d27171e005afa5`, migration revision `91f62d0a78659da2e8e264a6968b3c6cd32456de`, and revisions `da7c787d1e9f49110d7054868b146b5fb7d7bda4` and `0b86f07881198c57df1237b23a7d7c0084f2a272` retain their exact historical chains. Keep diagnostic, pre-migration, dirty, host-unverified, operator-observed, and rejected producer results visible without promoting them. Exact current and historical hashes, defect history, audit, review, and acceptance status belong in `docs/plans/pq2-clifford-string-qualification-progress-report.md`.
The first five proving groups passed on the controlled Linux AArch64 host under preceding performance inventories and remain historical after later inventory or shared-worker changes. Clean revision `5d226c94ece70f96d0b771f9c8cde7464ccd261b` closes the fifth group's historical AArch64 evidence chain without weakening the `1.25x` gate. Both under-specifications revealed by that audit are now resolved in `docs/plans/milestone-spec-gaps.md`: Stab allocation instrumentation covers every dense-XOR scale plus the accepted maximum, and clean implementation revision `b208a359f3f7676e2b07d64a5dc8caca208abf6a` adds completion receipt schema version 1 for every later executable slice. The sixth `not_zero`, seventh sparse-XOR, eighth BitMatrix transpose, ninth Pauli multiplication, tenth split Pauli iterator, and eleventh Clifford-string slices are complete on controlled Linux AArch64 at their recorded inventories; their progress reports record exact correctness, worker, report, regression, rollup, completion, audit, and review evidence. Clean revision `3f2f382627c8421de0a668819d467a9f252de20f` remains the latest complete 271-parent CQ2 execution checkpoint under its historical inventory; clean revision `859bf202bdd4bdfbca07e9b1d647afb1b0542846` supplies the accepted historical focused Clifford machine chain. Native x86-64 execution remains an unclaimed evidence target, and x86 adapter builds must prove that they inherit Stim's resolved machine flags before producing any ratio.
Keep PQ1's `pq1-adapter-protocol-smoke` ratio permanently diagnostic and never report it as product speed evidence.
Do not reopen CQ0 or PQ0 inventory semantics unless pinned-source drift, a newly exported default-feature API, a stale referenced id, a changed checklist or benchmark source of truth, or a confirmed inventory defect changes a frozen digest.
Do not treat PQ0's nine retained inherited rows as qualified evidence: the current inventory reports 158 missing correctness preflights, 158 missing output digests, 58 asymmetric CLI rows, 73 missing comparators, 123 missing scale families, and 20 heterogeneous selections. The nineteen implemented product contracts replace only their exact circuit parse, circuit canonical serialization, all-gate-name hash, toggle-plus-popcount, complete-vector dense XOR, three position-specific `not_zero`, sparse row XOR, sparse item toggle, allocating transpose, square in-place transpose, non-identity Pauli right multiplication, X/Z weight-range Pauli iteration, X/Y/Z singleton Pauli iteration, identity Clifford multiplication, complete-cycle non-identity Clifford multiplication, direct DEM parse, and direct DEM canonical serialization groups.

## Current Execution Contract

The eleventh Clifford-string PQ2 slice is closed on controlled Linux AArch64 at clean revision `859bf202bdd4bdfbca07e9b1d647afb1b0542846`. Preserve that report chain and every earlier accepted or review-rejected historical artifact under its recorded inventory; do not reopen completed groups merely to obtain a newer aggregate digest.

The active work is the post-migration closure of the twelfth PQ2 slice in `docs/plans/comprehensive-stim-performance-qualification-plan.md`. Implementation, exact correctness ownership, both adapter probes, worker reproducibility, the complete pre-migration machine chain, focused legacy migration, and retained memory-baseline checks are complete. Continue in this exact order:

1. Commit the schema-version-2 checked migration ledger and synchronized documentation as one clean source-contract revision. Verify `just bench::qualification-regenerate --check`, `just bench::qualification-check`, the complete `stab-bench` test suite, formatting, Clippy, benchmark smoke, and staged pre-commit before committing.
2. Do not edit source during the evidence chain. From that exact clean post-migration revision, rerun both DEM adapter probes, private-worker reproducibility, and the exact focused one-case CQ full producer, replay, and preflight into new unique artifact roots.
3. Produce all twelve post-migration timing reports: parse and print, full and soak, at small, medium, and large scales. Disable `/swap.img` only around each timing producer with an exit trap that always restores it, verify restoration immediately, and replay and regression-check each artifact before starting the next. Keep the first complete result; rerun only one complete group-scale attempt when the source-owned noise rule classifies the first as noisy.
4. Publish and replay separate parse-full, parse-soak, print-full, and print-soak architecture-scoped rollups from the passing reports, then publish and independently replay one parse completion and one print completion. Every artifact must bind the clean revision, current correctness digest, current performance digest, exact worker identities, and one controlled AArch64 CPU identity.
5. Update the DEM progress report with the exact post-migration revisions, artifact paths, hashes, ratios, confidence bounds, peak-RSS observations, replay and regression results, and any retained failed or noisy attempts. Explicitly distinguish source-current post-migration evidence from the pre-migration migration authorization.
6. Run `milestone-audit` over every twelfth-slice task, test, benchmark, migration, memory-retention, evidence, and acceptance criterion. Fix every confirmed implementation or evidence issue and rerun the affected machine chain from the fix revision. Log only genuine newly revealed under-specification in `docs/plans/milestone-spec-gaps.md`.
7. Spawn independent GPT-5.6/max `full-code-review` lanes for core DEM correctness, worker and adapter equivalence, hostile-input and artifact lifecycle, benchmark science and migration, and documentation consistency. Fix every confirmed P0 through P3 issue and regenerate affected evidence from the final clean revision.
8. Run full repository verification, commit focused fixes and final evidence documentation, and advance this execution contract only when both post-migration completion receipts replay, audit and review are closed, source-current docs agree, swap is restored, and the worktree is clean.

Do not infer completion of `PERF-DEM-MODEL`, deterministic PQ2, CQ3, PQ3, or the comprehensive program from this pair. Native Linux x86-64 execution and PQ6 memory-growth qualification remain separate obligations.

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
- Timed output must be consumed and work counters must be positive. Common-iteration groups require equal iteration counts, equal total work, and exact output digests before a ratio is computed. Source-owned independent-throughput groups require equal declared work per iteration, exact common semantic output at the smaller selected count, per-implementation repetition of the selected calibration output, and a normalized seconds-per-work ratio.
- A Stab zero-allocation timed-body claim requires allocator instrumentation at every runtime scale and the accepted maximum. Pinned Stim source inspection proves comparator shape only, not a Stim allocation count; cross-implementation allocation claims require instrumentation on both implementations, and process RSS remains separate PQ6 evidence.
- Full qualification uses calibrated batches, three warmups, nine interleaved paired samples, raw-sample retention, median paired ratios, relative median absolute deviation, and a fixed-seed bootstrap 95 percent confidence interval.
- PQ1 independently calibrates each implementation to a 350-millisecond target and a 2-second ceiling. A common equal-work validation batch normally requires both implementations between 250 milliseconds and 2 seconds. Wide-ratio mode may permit only the implementation that selected fewer independent iterations to exceed 2 seconds, while the common-iteration owner remains at or below 2 seconds and both remain between 250 milliseconds and the hard 20-second common ceiling. Only the checked Clifford identity contract may use independent-throughput mode: both selected calibration batches must stay between 350 milliseconds and 2 seconds, the smaller selected count must pass exact common semantics, and paired statistics must normalize each side by its exact work count. Later warmup and retained sample durations may jitter below 250 milliseconds or above 2 seconds, but each must remain positive, finish inside the fixed invocation timeout, repeat its selected count and output, and remain subject to the source-owned noise and threshold rules. Offline replay must derive every mode from the checked group policy, raw decisions, and receipts; callers and reports cannot select it.
- A primary row passes 1.25x only when both its median paired ratio and upper confidence bound are at most `1.25`.
- A slow comparable row cannot be waived.
- Noise classification must use paired-ratio relative MAD, not separate implementation-rate MAD. An initial noisy row receives exactly one complete group rerun with fresh warmups and the full sample count; retain both attempts and make the second authoritative regardless of outcome. Never rerun a non-noisy result or continue until favorable.
- A no-ratio disposition is allowed only when the validator proves that pinned Stim has no faithful comparator at the claimed surface and the reason names the condition that would retire it.
- Memory instrumentation cannot supply timing evidence.
- Process RSS comparisons, Stab allocation regressions, and scaling classifications must remain separate claims.
- Existing M12 thresholds remain active until replacement evidence is at least as strong and `benchmarks/qualification-threshold-migrations.json` machine-binds the exact authorization, migration, replacement target, and retained memory evidence.

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
- Formal correctness, performance, rollup, and completion producers must use unique previously absent output directories. Never erase a failed, noisy, host-rejected, or malformed attempt by rerunning into its path; only offline replay may compare-and-swap the derived bytes of the exact existing artifact.
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
just bench::qualification-probe --group pq2-dem-parse-adapter-smoke
just bench::qualification-probe --group pq2-dem-canonical-print-adapter-smoke
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
