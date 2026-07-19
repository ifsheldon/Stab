# Comprehensive Stim Performance Qualification Plan

## Status

Planned: 2026-07-13.

PQ0 completed: 2026-07-13, with source-owned evidence in [pq0-performance-disposition-progress-report.md](pq0-performance-disposition-progress-report.md).

PQ1 completed: 2026-07-14, with clean schema-version-13 PR, full, and soak evidence from commit `bfef511ccaa57c61cbe209c41d89d77ba8f52eee` recorded in [pq1-performance-harness-progress-report.md](pq1-performance-harness-progress-report.md). The bounded process runner, independent process and adapter probes, symmetric protocol-smoke workers, calibration, paired statistics, canonical CQ preflight reconstruction, host and current-toolchain policy, process-memory evidence, atomic reports, and report-only regression dispatch passed milestone audit and GPT-5.6/max review.

PQ2 is active after clean CQ2 completion; PQ3 through PQ7 remain planned.

Compatibility target: Stim v1.16.0 at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07` in `vendor/stim`.

Scope target: every implemented, non-deferred Rust and CLI contract identified by `docs/stab-feature-checklist.md` and every exported Rust API item that implements those selected contracts, with measurements for all meaningful variable-size work.

## Objective

Build a source-owned performance qualification suite that compares Stab with pinned Stim across the implemented product surface, exposes setup and execution costs separately, measures throughput, latency, memory, and scaling where each matters, and produces reproducible evidence without manufacturing ratios from unlike work.

The suite must retain the existing benchmark manifest and 1.25x primary gate as useful evidence, but it must requalify every inherited row against stricter workload-equivalence, runner-symmetry, measurement-pairing, and statistical-confidence rules before counting that row as comprehensive evidence.

## Meaning Of Comprehensive

The performance suite is comprehensive only when all of the following are true:

1. Every implemented selected feature and exported Rust API item has a stable performance disposition of `measured`, `covered-by-parent`, `not-performance-relevant`, or `no-faithful-stim-comparator`.
2. Every `measured` feature has at least one representative workload family, and every workload family declares its phase, scale points, work unit, correctness preflight, output-consumption rule, and memory policy.
3. Every claimed Stim ratio compares equivalent semantic work through a `stim-perf`, symmetric process CLI, or source-owned pinned-Stim adapter runner.
4. Every public CLI throughput ratio uses process-versus-process execution, while in-process CLI-body timings remain diagnostic and cannot stand in for end-to-end CLI parity.
5. Every multi-measurement row pairs exact named submeasurements or reports each measurement independently, and no heterogeneous row median is used as a performance claim.
6. Every timed workload verifies input identity, output width or count, and a semantic output digest before timing.
7. Every variable-size algorithm has small, medium, and large scale evidence or a documented mathematical reason why scale does not apply.
8. Every materializing path has peak-memory evidence at its largest supported workload, and every streaming or compact path has memory-growth evidence over at least three scales.
9. Every comparable primary row reports a paired ratio distribution and confidence interval against the frozen Stim build.
10. Every selected no-ratio row has a machine-checked reason proving that no faithful pinned-Stim comparator exists at the claimed surface.
11. The final report distinguishes suite completeness, 1.25x timing parity, memory regression status, and unresolved optimization work.

Comprehensive does not mean benchmarking deferred Python, JS/WASM, diagram, ecosystem, public interactive simulator, full ErrorMatcher provenance, exact random-stream, C++ header compatibility, QASM, Quirk, Crumble, or GPU products.

## Completion Versus Performance Parity

Suite completion and performance parity are separate conclusions.

- The suite is complete when every selected feature has a valid disposition and every measured group produces faithful, reproducible timing and resource evidence.
- A comparable row passes the performance target only when its declared gate statistic is at most `1.25x` pinned Stim.
- A slow comparable row is a valid benchmark result but a failed parity target; it must remain visible with an owner and profiler evidence and cannot be waived.
- A row can be `no-faithful-stim-comparator` only when pinned Stim lacks equivalent behavior at that surface and the suite validator proves that the disposition is still current.
- Existing M12 primary thresholds remain release-regression evidence until this plan explicitly graduates a replacement matrix.

## Sources Of Truth

- Feature boundary: `docs/stab-feature-checklist.md`.
- Upstream feature inventory: `docs/stim-feature-list.md` and `docs/plans/stim-test-porting-plan.md`.
- Correctness preconditions: `docs/plans/comprehensive-correctness-qualification-plan.md` and the planned `oracle/qualification-manifest.json`.
- Existing benchmark inventory: `benchmarks/manifest.csv`.
- Existing timing gates: `benchmarks/m12-primary-thresholds.json`, `benchmarks/m12-primary-beta-waivers.json`, and `benchmarks/m12-primary-regression-waivers.json`.
- Existing memory evidence: `benchmarks/m12-primary-memory-baseline.json`.
- Checked threshold-migration authorization: `benchmarks/qualification-threshold-migrations.json`.
- Existing runner implementation: `ops/bench` and `justfiles/bench.just`.
- Frozen upstream code and performance tests: `vendor/stim`.
- Planning lessons: `docs/plans/lessons-learned.md`.

If these sources disagree, the suite inventory must record the disagreement and the owning source must be corrected before the affected group can qualify.

## Planned Artifacts And Commands

### Performance Qualification Inventory

Add `benchmarks/stim-qualification-suite.json` as an overlay on `benchmarks/manifest.csv`.
The manifest remains the row-level workload source of truth, while the qualification inventory owns feature completeness, scale families, comparator fidelity, correctness dependencies, and gate policy.
Every selected checklist disposition must record exact selected-child ownership by performance domain; sharing a domain does not permit unrelated benchmark groups to claim the row or all of a partial row's children.

Each qualification group must include:

- `id`: stable benchmark qualification group id.
- `performance_feature`: exactly one primary feature id from the domain matrix in this plan, with every secondary API or inherited-row domain preserved in its source-owned supporting-feature list and corresponding parent group.
- `checklist_anchors`: exact section and row descriptions from `docs/stab-feature-checklist.md`.
- `checklist_child_ids`: exact selected child ids owned by a checklist group in its one performance domain; API and inherited-row groups must leave checklist ownership empty.
- `public_api_items`: exact rustdoc paths covered by the group or disposition.
- `disposition`: `measured`, `covered-by-parent`, `not-performance-relevant`, or `no-faithful-stim-comparator`.
- `reason`: required for every group so retained, reworked, diagnostic, superseded, removed, and replacement intent remains reviewable.
- `manifest_row` and `row_origin`: one nonempty stable primary row id classified as `inherited` when it exists in `benchmarks/manifest.csv` or `planned` when PQ0 owns a concrete future API, checklist, or resource workload.
- `phase`: `startup`, `parse`, `compile`, `execute`, `convert`, `serialize`, `search`, `transform`, or `end-to-end`.
- `runner_fidelity`: `stim-perf`, `adapter-library`, `process-cli`, or `stab-report-only`.
- `correctness_cases`: exact CQ0 owner ids that must pass before timing can run, or one stable `planned_correctness_case_id` when PQ0 has proved that no exact workload preflight exists yet; feature-level or truncated fallback cases are forbidden.
- `workload_family`: a typed repository-file, generated, or inline fixture locator; a deterministic seed or static corpus SHA-256; a registered generator id; exact small, medium, and large scale parameters; and a typed exact or not-applicable input-byte count at every scale.
- `work_unit`: bytes, bits, shots, gates, instructions, detector events, errors, flows, search nodes, or another named semantic unit.
- `output_contract`: expected output bytes, record count, width, semantic digest, and sink policy.
- `timing_policy`: warmup batches, paired samples, calibration bounds, timeout, and gate statistic.
- `memory_policy`: selected scales, allocation or process-RSS method, and expected growth class.
- `threshold_policy`: `primary-1.25`, `regression-only`, `report-only`, or `not-applicable`.
- `owner`: crate or subsystem responsible for regressions.
- `status`: `planned`, `implemented`, `qualified`, or `blocked`.

The inventory schema must deny unknown fields, validate all referenced feature, correctness, fixture, manifest, measurement, and waiver ids, reject unsafe paths and symlinks, bound all row and string counts before expensive work, and include a frozen semantic digest.
Benchmark source, fixture, stdin, and checked-output operations must use descriptor-relative nofollow traversal on qualification hosts; until equivalent non-Unix primitives are implemented, the ops binary must fail closed there instead of using path-check-then-open fallbacks.

### Pinned-Stim Adapter

Extend `ops/bench` with an ops-owned C++ adapter built from source under `benchmarks/stim_adapter/` when Stim's existing `stim_perf` filters and public CLI cannot expose an equivalent phase.

The adapter is benchmark infrastructure, not a Stab C++ API or C++ header compatibility product.
It may include pinned Stim internal headers only inside the adapter executable and must not add C++ compatibility promises to Stab.

The Rust ops binary must:

- Verify the exact submodule commit before building or running the adapter.
- Materialize the exact pinned Stim commit and exact committed adapter source into a fresh private build runtime for every qualification run without modifying `vendor/stim` or reusing a CMake cache.
- Use CMake or direct compiler invocations from Rust process APIs, with every command and relevant flag recorded in report metadata.
- Reject unsupported compilers, stale binaries, mismatched source digests, missing symbols, malformed protocol output, and nonzero adapter exits.
- Pass fixtures through validated repository-relative paths or bounded stdin, never through shell interpolation.
- Enforce per-workload timeout, stdout, stderr, row-count, and string-size limits.

The adapter protocol must be schema-versioned JSON Lines containing the workload id, measurement id, iteration count, elapsed seconds, semantic work count, output digest, peak memory when available, Stim commit, and build fingerprint.
Setup and fixture parsing must be outside the timed region unless the declared phase is `parse`, `startup`, or `end-to-end`.
Each adapter workload must have a Rust protocol test and an equivalence preflight against its corresponding Stab workload before it can produce a ratio.

Add a hidden bounded Stab qualification-worker mode to `stab-bench` that implements the same workload protocol for core comparisons.
The parent ops process must invoke the Stim adapter and Stab worker symmetrically for promotable core evidence, while existing in-process Stab runners remain useful local diagnostics.
The worker protocol must expose setup-complete resident memory, peak resident memory, allocation evidence when instrumented, and timing measurements from inside the worker so parent process overhead is not charged to one implementation only.

### Operational Surface

Keep the human-facing commands in `justfiles/bench.just` and complex logic in `ops/bench`:

```sh
just bench::qualification-list
just bench::qualification-check
just bench::qualification-regenerate --check
just bench::qualification-probe --group <id>
just bench::qualification-run --tier pr
just bench::qualification-run --tier full
just bench::qualification-run --tier soak
just bench::qualification-report --input target/benchmarks/qualification/latest
just bench::qualification-regression --baseline benchmarks/qualification-baseline.json
```

The existing `bench::baseline`, `bench::compare`, `bench::primary-beta`, `bench::primary-regression`, and `bench::primary-memory-regression` commands remain supported during migration.
No recipe may contain complex branching or a multiline shell implementation.

### Report Contract

Every JSON and Markdown report must record:

- Stab commit and `local_modifications`.
- Stim tag, commit, source digest, binary digest, and adapter digest when used.
- Rust and C++ compiler versions, build profiles, relevant flags, target triple, operating system, architecture, CPU model, logical CPU count, available memory, and CPU-affinity or governor status when observable.
- Tier, group filters, fixture digests, correctness preflight result, timeout policy, warmup count, sample count, calibration decisions, and run order.
- Runtime-group contract digest, immutable claim class, baseline eligibility, exact workload and measurement IDs, exact group-owned correctness cases, and controller-approved correctness request and completion digests for promotable groups.
- Raw sample durations, work counts, normalized rates, paired ratios, median paired ratio, relative median absolute deviation, and deterministic bootstrap confidence interval.
- Peak resident memory, allocation counts and bytes where available, memory-growth classification, and scaling slopes where required.
- Passed, failed, noisy, report-only, covered-by-parent, not-performance-relevant, and no-faithful-comparator counts by domain.
- Every ratio failure with its exact measurement pair, profiler-note path, and owning subsystem.
- Every no-ratio group with its machine-checked reason and follow-up condition.

Generated artifacts belong below `target/benchmarks/qualification/` and must never be treated as source-owned baselines merely because a local run succeeded. Every formal run, rollup, and completion producer must publish to a previously absent direct-child output directory and reject an existing path; only the corresponding offline replay command may compare-and-swap refreshed derived bytes into the exact existing artifact.

## Fair Comparison Policy

### Host Validity

- Add a source-owned `benchmarks/qualification-host-policy.json` schema describing required affinity support, minimum free memory, maximum pre-run load, swap-activity policy, and optional frequency-governor or thermal checks for promotable full evidence.
- Acquire an exclusive profile-and-selected-CPU qualification lease before initial host capture or private builds, retain it through final host capture and atomic publication, and fail closed when another qualification run holds the lease.
- Pin both workers or CLI processes in a pair to the same configured CPU set when the host policy requires affinity.
- Sample load, available memory, swap counters, frequency state, and thermal or throttling indicators before and after a group when the platform exposes them.
- Make offline report, preflight, and regression validation reload the exact source-owned policy, bind the recorded host identity and affinity to the validating host, and reconstruct the complete sorted violation set and `verified` outcome from recorded probes.
- Refuse source-owned promotion when a required host check fails or cannot be observed; local probes may continue only with an explicit `environment-unverified` status.
- Never combine samples across reboots, host fingerprints, CPU sets, power modes, or concurrent benchmark jobs into one ratio distribution.
- Record background-load and host-policy failures as environment failures instead of benchmark slowdowns.
- Maintain separate authoritative host profiles for Linux x86-64 and Linux AArch64 so portable-SIMD performance is qualified on both major CPU families.
- Run the full selected suite and apply architecture-scoped thresholds independently on each authoritative host profile; never combine or substitute ratios across architectures.
- Treat emulated architecture runs and ordinary shared-host CI runs as smoke or diagnostic evidence, not authoritative cross-architecture timing evidence.

### Build Equivalence

- The authoritative profile is the repository's production `release` profile for Stab and pinned Stim's documented `Release` build, with no user-injected target-feature override on only one implementation.
- An optional `native-release` diagnostic profile may enable native CPU tuning only when equivalent tuning is applied to both builds and the report identifies it as non-authoritative.
- Reports from different build profiles, CPU feature policies, Stim commits, or host architectures must never be combined into one ratio distribution.
- The runner must reject debug assertions, sanitizers, allocation tracking, profiling instrumentation, or logging in timing-gate builds unless the row explicitly measures that configuration.
- Memory runs use their own instrumented profile and cannot supply timing-gate evidence.

### Runner Symmetry

- Public CLI end-to-end rows must execute built `stab` and `stim` processes with equivalent arguments, inputs, environment, stdin or file mode, and output sink.
- In-process `stab_cli::run_from` rows may diagnose command-body costs but cannot be labeled process CLI parity.
- Core rows must time equivalent library phases through Stim's `stim_perf` runner or the pinned-Stim adapter and Stab's in-process runner.
- Process startup is measured as its own small-workload phase and must not be silently mixed into large-throughput core ratios.
- Compilation, reference-sample construction, allocation, execution, conversion, and serialization must be split when users can reuse the compiled object across calls.

### Input And Output Equivalence

- Both implementations receive byte-identical immutable fixtures or fixtures generated from the same source-owned parameters and digest.
- Randomized benchmark fixtures use fixed seeds and are materialized before timing.
- Every row performs an untimed correctness preflight that compares record count, width, semantic checksum, and exact output bytes where bytes are contractual.
- Timed outputs must be fully consumed through equivalent sinks and black-boxed so computation cannot be optimized away.
- Throughput rows may use an operating-system null sink only when both CLI processes use the same sink class and an untimed run has already verified output bytes.
- File-IO rows use separate same-filesystem scratch files, exclude explicit filesystem synchronization unless the public contract requires it, and report page-cache warmup policy.
- Input generation, fixture copying, temporary-directory creation, output hashing, and report serialization remain outside timing unless named by the phase.

### Correctness Evidence Handoff

- The performance consumer must independently reconstruct the current Oracle request, report, completion, preflight, and execution-receipt family before any product timing begins. A producer schema bump and its consumer support, mixed-family rejection tests, and a compact artifact family generated by the real Oracle controller belong in the same change set.
- Preserve only explicitly modeled historical schema families. Derive the expected execution-receipt and preflight versions from the report family and reject unsupported or mixed generations instead of accepting version fields independently.
- Reject merely planned or deferred prerequisites, passing cases without complete stdout and stderr receipts, retained failure artifacts, absent exit status, invalid Cargo exact-test counts, and report-to-receipt statistical disagreement. Reconstruct one global statistical attempt ledger across the complete passing result set, require exact planned and executed seed panels and shot totals, reject unknown or non-statistical owners and duplicate `(case_id, seed)` attempts, and derive the aggregate shots and seeds from nonzero passing attempts before accepting any per-case receipt.
- Source-current evidence is accepted for the producer and inventory contract at its recorded clean revision. Offline replay additionally requires that exact clean `HEAD`; a later documentation-only commit may leave the scientific checkpoint accepted without making its receipts replayable from the new checkout.
- Every promotable performance execution must consume focused exact correctness prerequisites generated from the same clean revision as the performance workers. A broad earlier all-domain checkpoint establishes program coverage but cannot replace this same-revision preflight.

### Timing And Statistics

- Calibrate each implementation independently to at least 250 milliseconds and at most 2 seconds without exceeding the workload's declared iteration or memory cap. PQ1 targets 350 milliseconds and records every raw calibration probe.
- Use the larger independently selected iteration count as the identical-work common batch unless the checked group contract explicitly selects `independent-throughput`. Standard mode requires both common-validation durations between 250 milliseconds and 2 seconds. When a genuine ratio has no standard overlap, wide-ratio mode may permit only the implementation that selected fewer independent iterations to exceed 2 seconds; the implementation that owns the common iteration count must remain at or below 2 seconds, both sides must remain at least 250 milliseconds, and neither may exceed the hard 20-second common ceiling under the 30-second invocation timeout. The 20-second bound covers the approximately 40x ratio implied by a 6-percent early-hit short circuit combined with the observed full-scan implementation advantage, while retaining 10 seconds of per-invocation timeout headroom. The only current `independent-throughput` owner is `PERFQ-M6-CLIFFORD-STRING`: each implementation uses its independently selected 350-millisecond through 2-second count, a common batch at the smaller selected count must prove exact semantics, every sample repeats its implementation-specific selected output digest, and ratios normalize both elapsed times by their exact declared work. Derive and offline-replay every mode from the checked group policy, raw calibration decisions, and receipts instead of accepting a caller or report classification.
- Use three unreported warmup batches before each complete qualification timing attempt.
- Use nine interleaved paired Stim and Stab samples for full qualification and fifteen for soak evidence.
- Alternate deterministic `Stim, Stab` and `Stab, Stim` order across pairs to reduce drift bias.
- Compute each pair as Stab seconds per work unit divided by Stim seconds per work unit.
- Report the median paired ratio and a fixed-seed 10,000-resample bootstrap 95 percent confidence interval over paired ratios.
- A primary 1.25x row passes only when both the median paired ratio and upper confidence bound are at most `1.25`.
- Report relative median absolute deviation for each implementation and for paired ratios.
- Do not delete outliers.
- If paired relative median absolute deviation exceeds 10 percent, mark the row noisy and require exactly one complete group rerun containing fresh warmups and the full pair count. Retain both attempts, make the second attempt authoritative regardless of its outcome, and never rerun a non-noisy result or continue rerunning until favorable.
- Keep per-implementation relative median absolute deviations as diagnostics, but do not use their common-mode rate variation to classify a paired ratio as noisy.
- Reject failed or noisy authoritative outcomes before applying source-owned numeric regression thresholds.
- Timeout, signal, malformed output, zero work, inconsistent digest, or incomplete sample evidence is a failed row, not a slow or waived row.

The PR tier may use three paired samples for smoke and regression direction, but it cannot mint a new qualification or threshold.

### Measurement Pairing And Aggregation

- Pair measurements by exact source-owned Stim and Stab ids.
- A renamed or missing measurement is a validation failure.
- A row with multiple comparable measurements passes only when every thresholded pair passes.
- Use the worst upper confidence bound as the row's gate summary.
- Never compute a median across parsing, compilation, execution, allocation, or serialization measurements.
- Report-only Stab extras remain visible but do not weaken or improve a paired ratio.
- A parent group may summarize child status counts but must not synthesize a timing ratio from heterogeneous children.

### Milestone Completion Receipts

- Every executable PQ2 product slice whose closure is claimed after completion receipt schema version 1 was introduced must publish one canonical machine-readable completion receipt before its progress report may claim closure. Earlier reviewed slices remain historical under their preceding acceptance contract and must not be relabeled as receipt-backed.
- Invoke `just bench::qualification-completion --group <group> --full-input <report> ... --soak-input <report> ... --full-rollup <rollup> --soak-rollup <rollup> --out <completion-directory>` from the same clean committed revision recorded by all source evidence. Every source report, rollup, and output must be a distinct direct child of `target/benchmarks/qualification/`.
- The completion controller invokes the same typed handlers as worker reproducibility, the source-owned adapter probe, report replay, regression, and rollup replay. Each typed step records the clean revision, canonical standalone argument vector, exact input artifact identities, command-equivalent exit status zero after successful handler return, typed result, and output artifact identities. A handler error aborts without publishing a receipt.
- Require exactly one promotable full and one promotable soak report per source-owned scale, an idempotent replay of every report and rollup, a passing source-owned regression result for every report, and passing full and soak rollups. Reject a repaired replay because a completion receipt must reconstruct from already valid evidence rather than conceal stale derived artifacts.
- Require private-worker reproducibility to produce the exact worker identity shared by every source report and rollup. Require the source-owned adapter probe to match the reports' Stim source, build, and binary identities and the Stab worker source identity. Require full and soak rollups to share exact host policy, host profile, CPU identity, architecture, target triple, toolchain, correctness preflight, and worker identity.
- Publish completion `report.json`, `preflight.json`, and derived `report.md` atomically into a previously absent output directory only after checks prove that every bound source `report.json`, `preflight.json`, and `report.md` remains unchanged. Reject an existing producer output path even when its bytes are invalid, failed, noisy, or host-rejected. Completion receipt schema version 1 and preflight schema version 1 bind the complete typed step sequence and all source directory artifact digests; only `qualification-completion-report` may compare-and-swap refresh an existing completion artifact.
- Invoke `just bench::qualification-completion-report --input <completion-directory>` to rerun every machine-checkable closure operation and require byte-identical receipt and preflight reconstruction. Human milestone audit and GPT-5.6/max full code review remain separately recorded human evidence and must not be represented as mechanically self-certified receipt steps.

### Memory And Scaling

- Process CLI rows measure peak resident set size for both Stab and Stim with the same process monitor and at least three repetitions per largest scale.
- Core rows selected for cross-implementation memory comparison run through the symmetric Stim adapter and Stab worker and report both setup-complete resident memory and peak resident-memory delta.
- In-process Stab rows may additionally record allocation count, total allocated bytes, and peak live bytes through the existing optional allocation tracker.
- Rust-only allocation evidence is a regression guard, not a Stab-versus-Stim memory ratio. A zero-allocation timed-body claim requires allocator instrumentation over every source-owned runtime scale and the accepted maximum, with fixture construction and output inspection outside the measured closure. Pinned Stim source inspection may establish that an isolated comparator performs the intended in-place operation and contains no explicit setup work, but it is not allocator evidence and must not be reported as a Stim zero-allocation claim. A future cross-implementation allocation claim requires C++ allocator instrumentation; process RSS remains separate PQ6 evidence.
- Streaming and compact-traversal rows use at least three geometrically increasing scales and fit a source-owned growth classification of constant scratch, linear in record width, linear in active state, or bounded materialization.
- Materialized rows verify the documented cap and measure the largest accepted workload plus the first rejected workload outside timing.
- Search workloads report search nodes, explored states, or another algorithmic work counter so a timeout or pruning change cannot masquerade as speed.
- Scaling failures are based on predeclared ratio or slope bounds, not visual inspection of charts.

## Qualification Tiers

### Schema Tier

Runs on every relevant change and validates inventories, exact ids, source anchors, fixtures, adapters, threshold coverage, waivers, and correctness dependencies without timing workloads.

### PR Tier

Runs a deterministic representative subset with short calibrated batches and three paired samples.
It catches broken runners, gross regressions, changed work, bad digests, missing measurements, and obviously unstable rows but does not create release claims.

### Full Tier

Runs every selected measured group with three warmups, nine paired samples, all required scales, process memory, and the current primary threshold matrix on controlled Linux x86-64 and Linux AArch64 benchmark host profiles.
Only a clean committed revision with `local_modifications=false` may become source-owned qualification evidence.

### Soak Tier

Runs adversarial large scales, fifteen paired samples, long-running search cases, streaming-memory slopes, and repeated randomized-but-seeded fixture families.
Soak catches nonlinear behavior and rare performance instability and does not replace the full tier.

## Performance Domain Matrix

### PERF-CIRCUIT-MODEL

- Parse small flat, medium tagged and noisy, large repeated, and large already-flattened circuits.
- Canonically serialize the same circuit families.
- Measure count and coordinate traversals separately from parse and serialization.
- Measure mutation, concatenation, repetition, flattening, noise removal, decomposition, unitary inversion, QEC inversion, feedback inlining, simplification, and flow time reversal only for implemented selected contracts.
- Include neutral huge repeats and bounded expanded repeats so folded traversal and materialization are not conflated.

### PERF-DEM-MODEL

- Parse and canonically serialize flat, sparse, coordinate-rich, nested-repeat, and large folded DEMs.
- Measure counts, adjusted traversal, selected coordinate queries, full coordinate maps, compact transforms, and bounded flattening separately.
- Include neutral huge repeats, zero-shift foldable repeats, sparse high detector ids, and error-heavy models.

### PERF-RESULT-IO

- Read and write `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64` through dense, sparse, wide, narrow, and 64-shot-aligned fixtures.
- Measure decode, encode, bounded stream conversion, side-output splitting, and writer-failure setup separately where applicable.
- Include dense-to-dense, dense-to-sparse, sparse-to-dense, typed `M/D/L`, raw-width, circuit-layout, DEM-layout, and observable side-output workloads.

### PERF-GATE-CONTRACT

- Measure canonical and alias lookup over hit and miss distributions.
- Measure validation for fixed-target, paired-target, Pauli-product, noisy, and annotation gates.
- Measure tableau, inverse, flow, unitary-matrix, and decomposition metadata only where construction performs nontrivial work.
- Static constant accessors may be `not-performance-relevant` when the inventory records why a parent lookup row covers them.

### PERF-BIT-KERNELS

- Compare XOR, AND, OR, clear, popcount, indexed gather or scatter used by Stab, 64-by-64 transpose, and sparse XOR kernels against equivalent pinned Stim operations. Add parity or raw random-fill rows only when a selected Stab bit-kernel API or engine hot path performs that exact work; typed Pauli, Clifford, and Tableau randomization belongs to `PERF-STABILIZER-ALGEBRA`.
- Exercise unaligned tails, word boundaries, 64-bit boundaries, sparse densities, dense crossover points, and portable-SIMD lane multiples.
- Keep scalar reference checks outside timing and record dispatch or feature selection once per run.

### PERF-STABILIZER-ALGEBRA

- Measure Pauli string multiplication, commutation, sign handling, sparse and dense application, tableau composition, inverse, gate append, tableau-to-Pauli conversions, flow multiplication, and flow validation for implemented Rust APIs.
- Use qubit widths spanning tiny latency, cache-resident throughput, and memory-bandwidth regimes.
- Require nontrivial separating inputs so identity cancellation cannot turn a benchmark into no work.
- Keep fallible constructor admission outside operation timing unless construction itself is the named workload. Every scale must stay within its source-owned `StabilizerResource` cap, cite exact CQ-ALGEBRA resource prerequisites, and distinguish value-size limits from tighter random-Tableau, solver, unitary-conversion, and aggregate flow-output algorithmic limits.

### PERF-GENERATION

- Measure core and process CLI generation for every implemented repetition-code, rotated surface-code, unrotated surface-code, and color-code task.
- Use at least two distances and two round counts per family, plus one noise-heavy configuration.
- Split circuit construction from canonical output serialization and include end-to-end `stab gen` versus `stim gen` process rows.

### PERF-CONVERT-CLI

- Requalify the existing M7 convert rows and add missing scale families for dense text, dense packed, sparse text, typed `dets`, `ptb64`, raw width, circuit layout, DEM layout, and observable side output.
- Replace in-process CLI parity claims with symmetric process rows while retaining in-process rows as diagnostics.
- For a Stab extension that pinned Stim rejects publicly, compare equivalent internal format work through the adapter when possible and keep only the public end-to-end extension row report-only.

### PERF-SAMPLING

- Split sampler compilation, reference-sample construction, one-shot latency, batch throughput, output conversion, and process CLI end-to-end sampling.
- Cover deterministic Clifford, noisy independent errors, correlated errors, repeated circuits, reset and measure-reset, MPP, pair measurements, heralded errors, observables, high qubit widths, and low versus high shot counts.
- Use fixed circuit and seed families, verify output shape and statistical correctness before timing, and report shots, measurements, and gate applications as separate work counters where useful.

### PERF-DETECTION

- Split detection-converter compilation, reference sampling, measurement-to-detection conversion, direct detector sampling, detector-frame execution, sweep record handling, feedback inlining, result encoding, and process CLI `detect` or `m2d` end-to-end paths.
- Cover `01`, `b8`, sparse formats, supported `ptb64`, detector-only output, appended observables, observable side output, skip-reference mode, and supported sweep-conditioned or ran-without-feedback subsets.
- Include high-shot narrow records, low-shot wide records, many detectors, many observables, and large folded repeat circuits.

### PERF-DEM-SAMPLING

- Split DEM sampler compilation, random-error selection, detector or observable accumulation, replayed-error parsing, sampled-error writing, result encoding, and process CLI `sample_dem` end-to-end paths.
- Cover independent, correlated, repeated, detector-only, observable-only, sparse, dense, replay, and side-output workloads.
- Include streaming large-shot scales and 64-shot `ptb64` groups without materializing all shots.

### PERF-ERROR-ANALYSIS

- Split circuit traversal, Pauli propagation, loop folding, graphlike decomposition, disjoint approximation, gauge handling, DEM serialization, and process CLI `analyze_errors`.
- Preserve the existing `m10-error-decomp` named submeasurements and add sizes and error shapes that expose sparse, dense, and decomposition-heavy behavior.
- Record analyzed instructions, error mechanisms, emitted DEM instructions, and decomposition attempts so output-shape changes cannot appear as speedups.

### PERF-SEARCH-AND-MATCHING

- Measure graphlike and hypergraph collection, shortest graphlike error, undetectable logical error search, SAT or WCNF collection and serialization, sparse reverse tracking, and selected matched-error filtering.
- Use solvable, unsatisfiable, bounded, and early-exit cases with declared node or state budgets.
- Compare minimum weight and semantic result digests before timing, and report search work counters alongside wall time.

### PERF-FLOWS-AND-DETECTOR-UTILITIES

- Measure flow generation, unsigned and selected signed checks, measurement solving, detecting-region construction, missing-detector checks, coordinate queries, and reverse-flow transforms.
- Cover unitary, measurement-rich, reset, MPP, observable, repeated, generated-code, sparse high-index, and batched-flow workloads.
- Keep generation, validation, solving, and transformation measurements separate.

### PERF-CLI-STARTUP-AND-ERRORS

- Measure process startup for `help`, a tiny parse-and-print command, and one representative command with no large output.
- Keep malformed-input and rejected-flag paths out of the throughput gate unless a denial-of-service or bounded-failure benchmark owns them.
- Add bounded hostile-input performance checks for parser nesting, giant counts, malformed sparse indices, path failures, writer failures, and search caps without comparing user-visible error text timing.

### PERF-RESOURCE-BOUNDARIES

- Measure process peak memory for representative public CLI families and Stab allocation behavior for selected in-process hot paths without mixing those evidence types.
- Measure streaming, compact traversal, materialization, and bounded-search growth across declared small, medium, and large scales.
- Exercise the largest accepted input and first rejected input for materialized and search caps outside the timing gate.
- Treat hostile-input latency as a bounded Stab regression unless pinned Stim performs the same accepted or rejected semantic work.

## PQ0: Freeze The Performance Disposition Ledger

Status: Complete.

Evidence: [pq0-performance-disposition-progress-report.md](pq0-performance-disposition-progress-report.md), the completed PQ2 slice reports, `benchmarks/stim-qualification-suite.json` at current schema-version-2 performance digest `c238dc4e2500192f310ef3d2378ecaafc9744662b5127784dd4eeb6c60726176`, and the checked `benchmarks/qualification-threshold-migrations.json` authorization ledger, all bound to correctness digest `4dbbb4b2cda3117bdd3d3ddfcd30b55f09e6f401352e3e86130222189d47791f`. Seventeen exact product runtime contracts are implemented with one exact `1.25x` measurement rule over each three-scale family, for 51 scale outcomes. The inventory digest binds the separate identity and non-identity Clifford-string contracts to scalable independent oracles, retired legacy identity/small timing provenance, normalized statistics, and independent-throughput timing only for the identity group. Runtime-group schema version 5 separately binds the shared source-owned optimization profiler note, and neither checked contract inherits timing evidence from an earlier inventory. Clean pre-migration Clifford revision `127d6661a9e00872fc4aa4c0b0d27171e005afa5` authorized the focused legacy timing retirement now reflected by this inventory; its exact report and preflight hashes are machine-bound by the migration ledger. The schema-version-30 post-migration chain from revision `91f62d0a78659da2e8e264a6968b3c6cd32456de` is accepted historical AArch64 evidence under its recorded inventory. Shared worker-source, hostile-request, publication, and migration-ledger changes require replacement schema-version-31 evidence before the Clifford ratios are source-current again. Clean reviewed iterator evidence revision `afaf0bf7f236b9f6ae6f72c19bbbdfea94d26632` remains accepted historical AArch64 evidence with 12 first-attempt passing reports and regressions, four replayed rollups, and two replayed completion receipts after exact accepted-maximum strengthening and clean final review. The memory baseline remains guarded for PQ6. Earlier completed groups remain historical under their recorded inventories.

Implementation revision: `abf7cd1bae0de045f62e976a290507238153f976`, verified with `local_modifications=false`.

### Objective

Turn the checklist and current 161-row manifest into a finite, reviewable inventory before adding more benchmark code.

### Tasks

1. Assign every implemented selected checklist feature a stable `PERF-*` feature id and performance disposition.
2. Assign every selected exported Rust API item from the CQ0 rustdoc inventory to a measured group, a measured parent, or an explicit non-performance disposition.
3. Map every existing `benchmarks/manifest.csv` row to exactly one primary qualification group and any supporting groups.
4. Classify each inherited row as faithful, diagnostic, proxy, stale, duplicate, missing scale, missing correctness preflight, or missing comparator.
5. Record exact upstream `stim_perf` filters, public CLI command shapes, or adapter symbols for every proposed ratio.
6. Identify every in-process-versus-process mismatch, heterogeneous median, missing output digest, unmatched submeasurement, and no-ratio waiver that an adapter can eliminate.
7. Freeze deterministic fixture families and size parameters without checking in unreviewably large generated files.
8. Add a machine-readable feature-to-performance coverage report and fail on missing or duplicate ownership.

### Tests

- Reject unknown feature, manifest, fixture, correctness, measurement, threshold, and waiver ids.
- Reject stale, duplicate, or unclassified exported Rust API paths and feature-gated API items absent from the declared build matrix.
- Reject a measured group without a primary row, phase, work unit, scale, output contract, or correctness dependency.
- Reject an API disposition that drops its CQ0 `owner_case_id` or any primary or secondary performance domain.
- Reject a planned primary row whose generator, seed, work unit, or small, medium, and large parameters say only to bind or decide them later.
- Reject a planned primary row whose generator is not registered for its group kind, whose exact scale ids or parameters drift, or whose typed input-byte count is absent or inconsistent with its work unit.
- Reject checklist groups that claim a domain-wide row set, a partial child outside their performance domain, a duplicate global `(child_id, performance_feature)` primary owner, or any checklist ownership from an inherited or API group.
- Reject repository fixtures without a bounded nonsymlink path, exact byte count, and SHA-256 corpus digest, including same-length content changes.
- Reject a heterogeneous row from `primary-1.25` when any selected Stim symbol lacks an exact named threshold pair.
- Reject `covered-by-parent` cycles and parents that are not measured.
- Reject `no-faithful-stim-comparator` when an existing Stim runner or adapter mapping is declared for the same contract.
- Reject a primary gate row backed only by an in-process-versus-process comparison.
- Snapshot only the stable coverage counts and unresolved classifications, not machine-specific timing values.

### Acceptance Criteria

- Every selected checklist row has exactly one performance disposition.
- Every partial checklist row has stable selected and deferred child ids, and every selected child domain has a concrete planned primary row.
- Every selected checklist child has explicit domain ownership, and only its exact checklist parent groups carry that ownership.
- Every behavioral API parent includes the exact CQ0 owner case and preserves all CQ0 performance domains.
- Every current manifest row has an explicit retained, reworked, diagnostic, superseded, or removed decision.
- The report contains no unowned row, missing feature, hidden waiver, or aggregate heterogeneous timing claim.
- The frozen inventory digest is reviewed before PQ1 runner work begins.

## PQ1: Build The Paired Qualification Harness And Stim Adapter

Completion evidence: [pq1-performance-harness-progress-report.md](pq1-performance-harness-progress-report.md) records the clean revision, final reports, pair counts, diagnostic ratios, commands, audit, review closure, and inherited M12 state.

Implementation note: `pq1-adapter-protocol-smoke` is a synthetic diagnostic group used only to prove the harness. It cannot accept product correctness evidence, enter a threshold baseline, or support a Stab-versus-Stim product speed claim.

Audit note: the parent must independently derive `iterations * work_items`, keep calibration probes work-bound and outside ratio evidence, perform semantic preflight at the exact source-owned common batch shape, bind every subsequent validation, warmup, sample, and memory receipt to the applicable common or implementation-specific digest, and inspect the clean revision through a config-free private Git view tied to an exact captured commit. Offline validation must replay the calibration algorithm from raw measured and process-wall durations, bind wrapper and row iterations, enforce the exact workload and measurement identities for every phase, derive standard, wide-ratio, or independently normalized mode from the checked group policy, independently selected iteration counts, and common receipts, reconstruct both per-implementation work rates, and reproduce repeated memory fields from raw invocation receipts. Both qualification workers must be rebuilt from materialized committed source in fresh private targets, bind canonical tool, argument, environment, input, fingerprint, and binary identities into reconstructable receipts, and execute from sealed copies. Controlled host evidence requires an exclusive full-run profile-and-CPU lease, stable thermal-zone identity and readings no higher than the profile limit whenever the platform exposes the required probes, and offline replay of the source-owned policy instead of trusting serialized `verified` or violation fields.

### Objective

Make faithful comparison, calibration, statistics, and reporting reusable before expanding workload coverage.

### Tasks

1. Implement the schema-versioned qualification inventory and validator in `ops/bench`.
2. Implement symmetric process CLI execution with bounded stdin, stdout, stderr, files, timeouts, and child cleanup.
3. Implement the pinned-Stim adapter and Stab qualification worker, including build or binary fingerprints, their shared JSON Lines protocol, and the bounded parent parser.
4. Implement deterministic batch calibration, three warmups, interleaved paired order, raw sample retention, normalized work rates, median paired ratios, relative median absolute deviation, and fixed-seed bootstrap intervals.
5. Implement exact submeasurement pairing and worst-upper-bound group summaries.
6. Implement correctness and output-digest preflight before timing, with exact correctness cases owned by a source runtime-group contract and externally approved CQ request and completion digests.
7. Implement host-policy validation, process peak-RSS and setup-baseline sampling, and existing Stab allocation tracking as separate evidence.
8. Add `qualification-list`, `qualification-check`, `qualification-probe`, `qualification-run`, `qualification-report`, and `qualification-regression` commands.

### Tests

- Unit-test calibration lower and upper bounds, zero-duration handling, overflow, timeouts, maximum iterations, standard common batches, valid wide-ratio batches in both implementation directions, source-owned independent-throughput batches, normalized unequal-count ratios, and floor, cap, equal-iteration, wrong-owner, work-unit, selected-output, and both-over-standard rejection.
- Unit-test deterministic `Stim, Stab` or `Stab, Stim` alternation and preserve all raw samples.
- Unit-test paired ratio, median, relative median absolute deviation, bootstrap interval, threshold boundary, and noisy-row classification against hand-computed fixtures.
- Unit-test common-mode Stim and Stab rate variation, noisy-rerun triggering, missing and untriggered reruns, wrong attempt reasons, second-attempt failure retention, and rejection of noisy authoritative regression evidence.
- Unit-test exact measurement pairing, stale ids, duplicate pairs, missing work, zero work, inconsistent work, inconsistent digest, and heterogeneous aggregate rejection.
- Unit-test hostile report mutations of calibration progression, wrapper and row iteration counts, phase workload or measurement ids, implementation and evidence mode, derived work, affinity, output digest, build identity, impossible wall duration, and repeated parent-RSS summaries.
- Integration-test process success, nonzero exit, signal termination, timeout, stdout or stderr overflow, writer failure, missing binary, and child cleanup.
- Integration-test adapter commit mismatch, source digest mismatch, stale binary fingerprint, malformed JSON, extra rows, missing fields, non-finite values, and oversized output.
- Integration-test Stab worker fingerprint mismatch, protocol drift, setup-memory ordering, worker panic, and parent-child work or digest disagreement.
- Integration-test host-policy pass, affinity failure, excessive load, insufficient memory, active swap, unavailable required probes, environment-unverified local mode, exclusive-lease contention and release, source-policy digest drift, and hostile report mutations that hide or fabricate violations.
- Test runtime-group duplicate, claim-class, baseline-eligibility, worker-shape, and correctness-case validation; baseline missing, unknown, stale, diagnostic-threshold, and incomplete-rule rejection; and externally approved CQ request and completion digest mismatch.
- Test that a memory-instrumented run cannot be consumed as timing-gate evidence.
- Test that a dirty worktree report cannot be promoted as source-owned final evidence.

### Acceptance Criteria

- Synthetic equal-speed workloads produce a confidence interval containing `1.0`.
- Synthetic 1.30x workloads fail the 1.25x gate without waiver support.
- Deliberately mismatched work or output never produces a ratio.
- Process CLI and adapter probes reproduce from one thin `just` command each.
- `just bench::smoke` succeeds, and existing M12 commands preserve their parsing, execution, report shapes, threshold files, waiver files, and failure semantics.
- Inherited M12 product-row failures do not fail PQ1 when the commands execute faithfully and leave those failures visible; PQ2 through PQ6 own replacing or graduating those rows with exact correctness prerequisites and equivalent-work evidence.

## PQ2: Qualify Models, Formats, Gates, Kernels, And Algebra

Status: Active as of 2026-07-19. All 271 selected CQ2 parents have complete exact ownership, and clean correctness revision `3f2f382627c8421de0a668819d467a9f252de20f` provides historical PR, full, and soak execution plus report replay and exact preflight under preceding correctness digest `4c940e983df10a7c95cc512939f4a0cce79f1865e141739af9378db581ea5f87`. Focused revision `ac20ffca` passed the exact three Clifford prerequisites under preceding digest `c50f27fd097ac870c987d1f91c44d9e6a75510ed4d51ec44853dbc328f0b1fa7`; packed-storage review-fix revision `9c672ef3c12c3fe68632e8609a58ae98714bc144` sets source-current correctness digest `4dbbb4b2cda3117bdd3d3ddfcd30b55f09e6f401352e3e86130222189d47791f`. Seventeen exact product runtime groups are implemented at performance inventory `c238dc4e2500192f310ef3d2378ecaafc9744662b5127784dd4eeb6c60726176`, including exact public in-place Pauli-string multiplication, the two split Pauli-string iterator workload shapes, and separate identity and complete non-identity Clifford-string multiplication. The Clifford groups bind the failed scalar diagnostics, packed portable-SIMD optimization, reviewed scalable oracles, retired legacy identity/small timing provenance, and source-owned independent-throughput policy for the identity group. Clean pre-migration revision `127d6661a9e00872fc4aa4c0b0d27171e005afa5` passed the complete two-group machine chain under preceding inventory `0ee3639389860799298164c94c647fcab45b03c9d67b941b1aad12c6e5e06df5` and authorized the focused migration; the checked migration ledger freezes that authorization. Clean post-migration revision `91f62d0a78659da2e8e264a6968b3c6cd32456de` passed and replayed the historical schema-version-30 exact CQ, worker, probe, timing, regression, rollup, and completion chain for both Clifford groups on controlled Linux AArch64. Current private Stab build-receipt schema version 5, adapter receipt schema version 11, contract-preflight schema version 12 with 212 probes, and qualification report schema version 31 require a fresh clean replacement chain. The immutable dirty diagnostic at `target/benchmarks/qualification/perfq-m6-clifford-identity-independent-schema30-review-final-20260719` also passes the normalized `1.25x` gate and offline replay at report SHA-256 `311f975a5b8789ca764d810168c1dfad95758f1f275a8921936d5b3c5228a07b`, but remains non-promotable because diagnostic-mode `--allow-unverified-host` was used. The ninth and tenth slices remain accepted historical Linux AArch64 evidence at their recorded revisions and inventories. Native x86-64, PQ6 memory growth, and all later runtime groups remain unclaimed.

### Objective

Cover the deterministic foundations that feed every higher-level workflow.

### Tasks

1. Qualify `PERF-CIRCUIT-MODEL`, `PERF-DEM-MODEL`, `PERF-RESULT-IO`, `PERF-GATE-CONTRACT`, `PERF-BIT-KERNELS`, and `PERF-STABILIZER-ALGEBRA`.
2. Port or adapt the relevant upstream C++ perf cases listed in `docs/plans/stim-test-porting-plan.md`.
3. Add adapter workloads only where no faithful existing `stim_perf` filter exposes the phase.
4. Add small, medium, and large scales and separate folded from materialized algorithms.
5. Add memory-growth evidence for result streaming, compact repeat traversal, and wide stabilizer structures.
6. Reclassify or remove stale M4 through M6 rows only after their replacement evidence is present.

### First Executable Slice

1. Generalize the schema-version-4 runtime group ledger so every group owns one or more immutable named scales, positive semantic work counts, exact input byte counts and digests, an implementation owner, any source-owned profiler-note contract, and any exact comparator-source paths and digests.
2. Make `qualification-run --group <id> --scale <id>` resolve the complete scale identity from that ledger and reject unknown groups, unknown scales, caller-selected replacement work counts, stale report scale ids, work-count mismatches, and input byte or digest drift.
3. Implement `PERFQ-M4-CIRCUIT-PARSE` first with one `parse` measurement and `small`, `medium`, and `large` scales of 64, 4,096, and 65,536 instructions.
4. Bind the group to `cq-evidence-qualification-633fa529edf5f549` and `cq-evidence-qualification-e660819ae9a223c6`, which own Stim-text construction and canonical round-trip behavior.
5. Generate the deterministic six-instruction fixture cycle outside the timer, measure only repeated parse and replacement of the previous parsed circuit, and derive the semantic digest from the final parsed canonical circuit outside the timer.
6. Normalize only the known single terminal-newline difference between Stab canonical circuit text and pinned Stim `Circuit::str()` before digesting. Any other canonical difference blocks timing.
7. Cap the circuit-parse fixture at 1,000,000 instructions before allocation in both workers and reject the first unsupported instruction count, while assigning maximum-accepted and 1,000,001-instruction resource evidence to PQ6 instead of treating the 65,536-instruction timing scale as cap evidence.
8. Bind every report to the exact runtime and checked-inventory contract, retain setup and peak RSS separately, and require failed or noisy promotable evidence to carry the source-owned owner and profiler-note path and digest through offline replay.
9. Derive report promotion from the evidence: product PR, dirty, or unverified-host reports may remain valid diagnostics with exact CQ preflight, but only clean verified full or soak reports are promotable and eligible for regression dispatch.
10. Keep the PQ1 protocol-smoke default group and `default` scale for command compatibility, but never migrate its diagnostic ratio into a product threshold.
11. Publish each scale-family rollup only from a clean unchanged checkout whose commit exactly equals the source reports' Stab commit and into a previously absent output directory, record that producer state separately from the source-report identity, require exact Stim and Stab worker source, build-fingerprint, and binary-digest identity across every scale, and bind each source report and preflight while retaining only the reduced rollup evidence needed for the family. Reject an existing output even when it contains a failed, noisy, unverified-host, or malformed result.
12. Add offline rollup replay that reopens the current checked inventories, runtime group, canonical rollup and preflight, every exact source report and preflight, and the clean producer revision; reconstruct the complete canonical JSON and derived Markdown; reject altered paths, source bindings, outcomes, counts, identities, or preflight bytes; and use compare-and-swap publication so stale validation cannot replace newer evidence.

The first slice is infrastructure plus one proving workload. It graduates exactly `PERFQ-M4-CIRCUIT-PARSE` into the checked performance inventory and reclassifies the inherited `m4-circuit-parse` row from retained to reworked because the exact replacement contract exists. Source-current clean AArch64 full and soak evidence and both scale-family rollups at revision `ba70a52025fdd4122ac97cec263725b2ec56e431` pass the unchanged `1.25x` target, with full and soak medians of `0.920317x` and `0.920661x` at small scale, `0.897744x` and `0.900131x` at medium scale, and `0.963578x` and `0.970298x` at large scale. This slice does not complete `PERF-CIRCUIT-MODEL`, satisfy PQ2's remaining planned groups, or provide native x86-64 evidence.

### Second Executable Slice

1. Graduate `PERFQ-M4-CIRCUIT-CANONICAL-PRINT` as a separate `serialize` phase with the same exact 64, 4,096, and 65,536-instruction fixture family as circuit parsing.
2. Bind the group to `cq-evidence-qualification-e660819ae9a223c6` and `cq-evidence-qualification-ef933925fb901877`, which own canonical round-trip and canonical-printer behavior.
3. Construct and parse the exact fixture once before the start barrier in both workers, then time only repeated `Circuit::str()` and `Circuit::to_stim_string()` calls.
4. Consume every produced string so the optimizer cannot remove intermediate serialization, retain the final string, and compute its semantic digest outside timing.
5. Normalize only Stab's single terminal newline before comparing exact output bytes; reject any other output difference before a ratio is produced.
6. Count one semantic work item per serialized circuit instruction, retain output allocation and destruction in the measured body, and keep fixture construction and parsing in setup.
7. Record process setup and peak RSS separately at every scale. The shared one-million-instruction accepted boundary and first rejection remain PQ6 resource evidence.
8. Add one exact `1.25x` median and upper-confidence-bound regression rule for `serialize`; do not retire the legacy M12 contract-only waiver until current clean replacement evidence and migration documentation are complete.
9. Rebuild both private workers twice from one clean commit, run the exact adapter probe, and produce full and soak source reports plus architecture-scoped rollups for canonical print.
10. Regenerate parser reports and rollups from the same clean worker and inventory so the first group remains source-current after the shared worker extension.

The second slice is complete on controlled Linux AArch64 at performance inventory `f3c4009044b0bafcd877f76798c7f4f08c475c0877b85f68d22ae0449e3ddb8f` and correctness inventory `b80801fea6eae550feecf40489259de56123f6f3331b747d52c323d576fd0285`. Clean revision `ba70a52025fdd4122ac97cec263725b2ec56e431` binds one reproducible private-worker identity, one exact three-case correctness execution, six printer reports, six refreshed parser reports, twelve successful regression checks, and four replayed AArch64 rollups. Canonical-print medians are `0.375252x` and `0.373080x` at small scale, `0.372912x` and `0.376075x` at medium scale, and `0.373970x` and `0.375580x` at large scale for full and soak respectively. The legacy `m4-circuit-canonical-print` row is superseded, removed from the M12 beta and timing-regression waivers and memory baseline, and retained only as a non-primary historical diagnostic. Native x86-64 execution remains unclaimed, programmatic nesting beyond the parser's 256-level admission limit remains CQ6/PQ6 resource work, and the flat fixture does not qualify float-heavy, tag-heavy, target-heavy, repeat-heavy, or public file-output performance.

### Third Executable Slice

1. Graduate `PERFQ-M4-GATE-LOOKUP` as a single `hash-all-names` execute-phase measurement derived from pinned Stim's `gate_data_hash_all_gate_names` workload.
2. Bind the group exactly to `cq-evidence-qualification-bd20a013e903a05f`, whose independently selectable selector freezes the ordered 82-entry Stim v1.16.0 name table including `NOT_A_GATE` and every per-name hash value; aliases, lowercase resolution, and invalid-name rejection remain separately owned lookup behavior.
3. Prepare the 82-entry Stim v1.16.0 name table, including `NOT_A_GATE`, outside timing in both workers. Reject work counts that are not complete 82-name table sweeps.
4. Use scales of 82, 5,248, and 335,872 hashes, corresponding to 1, 64, and 4,096 complete table sweeps. Bind zero input bytes and the exact empty-input digest at every scale.
5. Time only `gate_name_to_hash` and `Gate::stim_name_hash` over the prepared runtime-owned names, place one symmetric compiler fence per complete sweep, preserve wrapping checksum accumulation, and compare the final checksum plus an untimed order-sensitive name-and-hash table fingerprint before producing a ratio.
6. Keep alias, lowercase, and invalid-name lookup measurements in the legacy M12 diagnostics; they have no equivalent pinned Stim perf symbol and cannot be aggregated into this ratio.
7. Record setup and peak process RSS separately at every scale as report-only observations. This slice makes no bounded-growth acceptance claim; PQ6 must define and validate explicit cross-scale RSS and allocation slack before memory qualification.
8. Apply the unchanged `1.25x` median and bootstrap upper-confidence-bound threshold at all three scales, with no waiver path.
9. From one clean committed revision, run the exact CQ preflight, worker reproducibility, adapter probe, full and soak scale families, regression replay, and AArch64 rollups.
10. Run milestone audit and GPT-5.6/max full code review before recording completion evidence. Keep any failed or noisy ratio visible with a profiler note and owner action.

The third slice is complete on controlled Linux AArch64 at performance inventory `1cc0be5c8c0a37c98bd4fb56f331dd6964e6f53e56b328b9564be507cbf88a42` and correctness inventory `ccb80eb4b660a375b59460c3b7fa03a810abd6f868735b566735378105db22b2`. Clean revision `c76b7071fc4d7ac358ef3a2fffc053ea675bd05f` binds one exact passing CQ report and preflight, one reproducible six-digest worker identity, six passing non-noisy source reports, six successful regression replays, and two replayed AArch64 scale-family rollups. Median ratios range from `0.931886x` to `0.932764x`, with worst confidence-interval upper bound `0.933289x`. Setup and peak RSS remain report-only observations until PQ6 defines and validates an explicit cross-scale growth rule. Native x86-64 execution and the remaining PQ2 groups remain unclaimed.

### Fourth Executable Slice

1. Graduate `PERFQ-M5-SIMD-WORD` as one execute-phase `toggle-popcount` measurement derived from pinned Stim's `simd_compat_popcnt` workload. The Stim comparator must reproduce the architecture-dependent upstream loop exactly by toggling bit 300 and accumulating `ptr_simd[k].popcount()` for every `k < num_simd_words`; calling the broader `simd_bits::popcnt()` helper is not an equivalent source contract on architectures where `MAX_BITWORD_WIDTH` exceeds 64. Bind the exact adapter call site and isolated comparator implementation by repository path and SHA-256 in the generated inventory, runtime group contract, materialized adapter receipt, and report replay. Do not combine this group with XOR, nonzero scans, masked operations, copies, or sparse-vector work.
2. Bind the group to `cq-evidence-qualification-5118006702599a45`, `cq-evidence-qualification-b1530dc4e48e942d`, and `cq-evidence-qualification-ba252d42660a41ce`, which own scalar-word popcount, logical-vector popcount, tail handling, and in-range bit access. Every selector must pass under the current correctness digest before timing.
3. Generate identical little-endian `u64` fixture words in both workers with the source-owned `splitmix64-word-v1` function. Construct Stim `simd_bits<MAX_BITWORD_WIDTH>` and Stab `BitVec` values before the start barrier and release temporary fixture storage before setup RSS is sampled. Build the standalone adapter with the exact CMake-generated `libstim` compile flags, including resolved machine flags, and bind that ordered flag list into receipt validation and the adapter build fingerprint so architecture-dependent headers and the linked library cannot disagree.
4. Use exact aligned bit-width scales of 4,096, 262,144, and 16,777,216 bits. Bind input sizes of 512, 32,768, and 2,097,152 bytes and the exact generated input digest at every scale.
5. Time only one compiler fence, toggling bit 300, popcounting the complete vector, and wrapping checksum accumulation per iteration. Prepare the initial toggle state before timing and read the final vector state, construct output fields, and digest those fields after timing. Count every visited bit as semantic work and keep allocation, fixture generation, input hashing, branch-specific fixture lookup, and output construction outside timing.
6. Accumulate every popcount into a wrapping checksum. Encode exactly eight `u64` fields in this order: checksum, iteration count, bit width, all four fixture-fingerprint lanes in lane order, and final toggle state as zero or one. Encode each field in little-endian byte order, run the shared four-lane byte-digest algorithm over the resulting 64 bytes, and emit the four digest lanes as 16-digit lowercase hexadecimal values in lane order. The parent must reject any Stim and Stab work, input, or output disagreement before producing a ratio.
7. Reject widths below 512 bits, widths not divisible by 256 bits, and widths above 268,435,456 bits before allocation and before reading the start barrier. Preparing workers for every qualification run must execute both sealed workers at the first below-minimum aligned width, one in-range unaligned width, and the first over-cap aligned width with no barrier input and require the exact source-owned errors. It must also execute the shared frozen protocol vector, fixed odd and even popcount vectors, and the accepted 268,435,456-bit maximum. Record all 18 actual accepted or rejected probe receipts in the report, include both workers' exact source, build-fingerprint, and binary digests in the preflight digest material, and make offline replay compare the receipts, recomputed digest, and six worker identities against the source-owned contract and report worker evidence. Standalone worker reproducibility repeats that complete contract for two isolated builds and requires identical input, output, preflight, source, build, and binary identities.
8. Record setup and peak process RSS separately at all three scales as report-only observations. This slice makes no linear-growth or Stim-relative memory acceptance claim; PQ6 owns explicit cross-scale RSS and allocation slack.
9. Apply the unchanged `1.25x` median and bootstrap upper-confidence-bound threshold to `toggle-popcount` at every scale with no waiver path. If any clean result fails or is noisy, retain it with a source-owned profiler note and owner action.
10. Reclassify the inherited `m5-simd-word` row from retained to reworked when the exact replacement contract enters the checked inventory. Keep its existing M12 threshold active until later migration evidence explicitly supersedes it; this slice must not silently remove the older guard.
11. From one clean committed revision, run all three exact CQ prerequisites, private-worker reproducibility, the adapter probe, full and soak source reports at every scale, regression replay, and separate AArch64 rollups. Run milestone audit and GPT-5.6/max full code review before recording completion evidence.

The fourth runtime contract is complete on controlled Linux AArch64 at performance inventory `877df12bf1b3d63da92289e22f117097cedbc20860d165c90b41554aa110263b` and correctness inventory `ccb80eb4b660a375b59460c3b7fa03a810abd6f868735b566735378105db22b2`. Initial full-tier reports from clean revision `38a2d5eab2fec3211eb9466899c6afd0ba91c4ca` and one later small report from revision `238cf3429e25aa6ed63dce716ed3c14f9ed5f5b3` remain rejected diagnostic history because milestone audit and GPT-5.6/max review found comparator, output identity, timing-boundary, frozen-vector, and cross-worker receipt-binding defects. Clean revision `56dfe7569c6da48ffe76bde18f21ff43095f029b` closes those defects and binds three exact CQ prerequisites, receipt-bound CMake flags, both comparator-source digests, one shared frozen protocol vector, 18 actual probes, six worker identities, reproducible sealed builds, six passing non-noisy source reports, six regression replays, and separate replayed full and soak AArch64 rollups. Median ratios range from `0.488067x` to `0.545545x`, with worst confidence-interval upper bound `0.547441x`. The earlier gate-name-hash timing reports remain valid historical evidence under performance inventory `1cc0be5c8c0a37c98bd4fb56f331dd6964e6f53e56b328b9564be507cbf88a42`; they are not relabeled as current after the shared workers and global performance inventory changed. Native Linux x86-64 execution and all remaining PQ2 groups remain unclaimed.

### Fifth Executable Slice

1. Graduate `PERFQ-M5-SIMD-BITS` as exactly one execute-phase `xor-complete-vector` measurement derived from pinned Stim's `simd_bits_xor_10K` workload. The timed Stim body must call `destination ^= source`, and the timed Stab body must call `BitVec::xor_assign` over the same complete aligned width. Do not aggregate `simd_bits_not_zero_100K`, randomization, masked XOR, range XOR, copying, clearing, or other logical operations into this ratio.
2. Bind the group to `cq-evidence-qualification-b1530dc4e48e942d` and `cq-evidence-qualification-ba252d42660a41ce`, which own complete-vector XOR semantics, canonical tails, zero and nonzero behavior, length rejection, allocation-free in-place mutation, storage shape, and access boundaries. Both exact selectors must pass under the current correctness digest before timing.
3. Replace the adapter receipt's single special-case comparator digest with a schema-version-5 ordered typed comparator-source collection. The receipt, build fingerprint, private build, generated inventory, runtime group contract, report, and offline replay must bind `benchmarks/stim_adapter/main.cc`, the isolated popcount comparator, and a new isolated dense-XOR comparator independently. Reject missing, extra, duplicate, reordered, path-altered, content-altered, or cross-receipt comparator sources. Existing groups may select the exact comparator-source subset they require, but the sealed adapter build must remain bound to every comparator source compiled into it. Preserve CMake's resolved `libstim` flags, treat pinned Stim's headers as external headers through `-isystem`, and retain `-Wextra -Werror` for the adapter-owned translation unit so upstream header warnings cannot weaken adapter warning enforcement.
4. Generate two equal-width vectors outside timing with `splitmix64-xor-pair-v1`: destination word `k` uses the existing SplitMix64 word function at index `2*k`, and source word `k` uses it at index `2*k+1`. Hash the exact little-endian destination bytes followed by the exact little-endian source bytes. Use aligned widths of 4,096, 262,144, and 16,777,216 bits, exact combined input byte counts of 1,024, 65,536, and 4,194,304 bytes, and source-owned input digests in both the checked inventory and runtime contract.
5. Prepare both vectors, input bytes, and input digest before the start barrier. Each timed iteration must execute one `std::atomic_signal_fence(std::memory_order_seq_cst)` before Stim's XOR and one `compiler_fence(Ordering::SeqCst)` before Stab's XOR; no other anti-elision work belongs inside timing. Keep allocation, fixture generation, validation, hashing, output construction, and final-state inspection outside timing. Count one semantic work item per destination bit visited per iteration.
6. After timing, hash the complete final destination and unchanged source vectors. Construct the canonical semantic output from fourteen little-endian `u64` fields in this order: iteration count, bit width, all four input-fingerprint lanes, all four final-destination fingerprint lanes, and all four final-source fingerprint lanes. The parent must reject any work-count, input-byte, input-digest, or output-digest mismatch before producing a ratio.
7. Use fixed one-iteration and two-iteration 4,096-bit vectors to prove odd and even final states and fixed canonical output digests. Execute one iteration at the accepted maximum of 268,435,456 bits. Reject widths below 256 bits, widths not divisible by 256 bits, and widths above 268,435,456 bits before allocation and before reading the start barrier. Contract-preflight schema version 4 must record all 30 actual accepted and rejected receipts for both sealed workers, qualification report schema version 20 must bind that expanded preflight and private-worker source-collection receipt, and standalone reproducibility must repeat the complete preflight for two isolated builds.
8. Record setup and peak process RSS separately at every scale as report-only observations. Two preallocated vectors must remain live during timing, the source must remain unchanged, and allocator instrumentation must prove that the Stab timed mutation allocates zero calls and zero bytes at the small, medium, large, and accepted-maximum widths. Pinned Stim source inspection proves only the isolated in-place comparator shape and does not establish a Stim allocation count. This slice makes no cross-scale RSS, cross-implementation allocation, or Stim-relative memory claim; PQ6 owns the explicit growth rule.
9. Apply the unchanged `1.25x` median and bootstrap upper-confidence-bound threshold to `xor-complete-vector` at every scale with no waiver path. A failed or noisy clean result must remain visible with a source-owned profiler note and next owner action.
10. Reclassify the inherited heterogeneous `m5-simd-bits` row from retained to reworked only after the exact XOR replacement enters the checked inventory. Keep the existing M12 XOR and `not_zero` submeasurement thresholds active until later migration evidence explicitly replaces each one, and do not imply that this XOR slice qualifies `not_zero` or the unmatched Stab-only logical operations.
11. Split production helpers or tests before `worker.rs`, `invocation.rs`, or `adapter.rs` crosses 1,200 lines. Any helper moved out of the sealed worker or adapter source must be included in the corresponding source identity and build fingerprint instead of becoming unbound executable behavior.
12. From one clean committed revision, run both exact CQ prerequisites, private-worker reproducibility, the dense-XOR adapter probe, full and soak reports at every scale, immediate offline replay and regression checks, and separate AArch64 scale-family rollups. Run milestone audit and GPT-5.6/max full code review as separately recorded human evidence before recording completion. This fifth-slice contract predates completion receipt schema version 1 and is not retroactively changed by that later requirement.

The fifth slice owns one dense XOR phase only. `simd_bits_not_zero_100K`, randomization, masked and ranged mutation, copy, clear, bit-table operations, sparse XOR, and all other remaining bit-kernel groups stay planned or retain their exact historical diagnostic evidence until separately graduated.

The fifth runtime contract is complete on controlled Linux AArch64 at performance inventory `fb50789c58786219c807c79e87202396b17563ee7cb584bcda4d3379007ed716` and correctness inventory `ccb80eb4b660a375b59460c3b7fa03a810abd6f868735b566735378105db22b2`. Clean revision `5d226c94ece70f96d0b771f9c8cde7464ccd261b` binds both exact CQ prerequisites, the ordered typed comparator-source receipt, 30 actual contract probes, reproducible sealed workers, six passing non-noisy source reports, six successful report replays and regression checks, and separate replayed full and soak AArch64 rollups. Median ratios range from `0.374633x` to `0.559828x`, with worst confidence-interval upper bound `0.561257x`. Milestone audit found no implementation, comparator-fidelity, schema, resource-boundary, M12-threshold, or performance-gate defect; it found and closed command-record and documentation-synchronization defects and logged two genuine under-specifications. The independent GPT-5.6/max full code review found and closed stale current-evidence wording, an incorrect digest label, and an incorrect CQ-owner attribution, with no confirmed code or evidence-integrity defect. Native Linux x86-64 execution and all remaining PQ2 groups remain unclaimed.

The post-fifth-slice performance inventory was `8b4735f7d651e74d3029014a4bf0c4580d85462295d66c93d8e090d3433c3958`. It added an exact source-owned replacement contract from the legacy `simd_bits_xor_10K` and `stab_simd_bits_xor_10K` threshold pair to executable measurement `PERFQ-M5-SIMD-BITS/xor-complete-vector`. Validation rejects stale or duplicate source pairs, duplicate targets, cross-feature or nonpromotable targets, missing primary mappings, and runtime measurement IDs absent from the executable group contract. At that checkpoint, the unmapped `not_zero` threshold remained active. Qualification report schema version 20 embedded private Stab build-receipt schema version 2, whose ordered framed source collection bound `worker.rs` and its extracted bit-kernel module. Stab zero-allocation evidence covered every dense-XOR runtime scale and the accepted maximum; no Stim allocation claim was made. Completion receipt schema version 1 closed the machine-readable command-sequence boundary for future executable slices. Historical dense-XOR evidence predates this schema and remains historical rather than being retroactively described as receipt-backed.

### Sixth Executable Slice

1. Graduate exactly three execute-phase contracts: `PERFQ-M5-SIMD-BITS-NOT-ZERO-EARLY`, `PERFQ-M5-SIMD-BITS-NOT-ZERO-ALL-ZERO`, and `PERFQ-M5-SIMD-BITS-NOT-ZERO-LATE`. Each owns one `not-zero` measurement and one input-position class. Never aggregate their times or outcomes because early termination performs different work from a zero or late-hit full scan.
2. Derive the early contract from pinned Stim symbol `simd_bits_not_zero_100K`. The upstream symbol is mislabeled: its source constructs `10 * 1000` logical bits and sets bit 600, exactly 6 percent of the logical width. Preserve the symbol name as provenance but freeze 10,000 bits and hit position `bits * 3 / 50` as the actual contract. Add all-zero and final-logical-bit patterns as source-owned adversarial companions so the upstream early-hit case cannot hide full-scan regressions.
3. Bind all three groups to exact CQ2 owners `cq-evidence-qualification-b1530dc4e48e942d` and `cq-evidence-qualification-ba252d42660a41ce`. These cases own `BitVec::not_zero`, zero and nonzero semantics, canonical tail behavior, storage shape, and access boundaries. Both cases must pass from the same clean revision and current correctness digest before any ratio is promotable.
4. Use logical bit widths of 10,000, 640,000, and 40,960,000 for `small`, `medium`, and `large`. Materialize exactly `ceil(bits / 64)` little-endian logical words, clear every Stim padded word, and set at most one logical bit. Bind exact input byte counts and digests independently for every pattern.

| Pattern | Scale widths | Input bytes | Input digests, small / medium / large |
| --- | --- | --- | --- |
| early | 10,000 / 640,000 / 40,960,000 | 1,256 / 80,000 / 5,120,000 | `652aebf153201450c8fe9d3707aed8cb0ee9fee8f5332d88e2001c56cfd0838f` / `f2af8de388713368d12e7bf4188e96c030bf1c3e2906250672e2f2eee9370aa8` / `84118644943bed7c2aa82daafc7e8b8f2358d0e38ab07fd140c8aba466fb3ba4` |
| all-zero | 10,000 / 640,000 / 40,960,000 | 1,256 / 80,000 / 5,120,000 | `b6286dfe1dca80e14e17bbc6a371565900665697e8f4f2b22d30a303f804b537` / `60aace21d864e2176a3f43edcd21a970c401e36a0223c24d09a8d482e075aae0` / `080543f5fd6fe5ca816fbfc568988f74eb08c7477f433ccbdecbc16d62790ec8` |
| late | 10,000 / 640,000 / 40,960,000 | 1,256 / 80,000 / 5,120,000 | `76618d8f234d913b3b6f99be0c83fca1e8a6eb3c5cdb6f622c06dccc7aaa2cc0` / `61aace21da17e2176a3f445b0d21a9b0c41d536a0223c24deda8d482e075aae6` / `0b0543f60288e5ca816fc551a8988eb4e96d37477f433ccbe2cbc16d62790f06` |

5. Construct vectors, validate widths, and hash inputs before the start barrier. Time only repeated `simd_bits::not_zero()` or `BitVec::not_zero()` calls, with a matching compiler fence and optimizer-opaque immutable input reference before every call plus wrapping Boolean checksum accumulation. Keep fixture lookup, allocation, digest construction, and output formatting outside timing. Count the declared logical bit width per iteration as the stable throughput denominator while retaining the pattern in the group identity; do not reinterpret that denominator as proof that an early-hit call physically scans every bit.
6. Encode exactly eight little-endian `u64` output fields: checksum, iteration count, logical bit width, pattern marker, and all four input-digest lanes. The marker is the early or late hit index and `u64::MAX` for all-zero. Freeze two-iteration outputs for all three 10,000-bit patterns and a one-iteration late output at the accepted maximum; both workers must match all input and output receipts exactly before timing can start.
7. Accept every logical width from 64 through 268,435,456 bits, including widths not divisible by 64 or 256. Reject 63 and 268,435,457 before allocation and before consuming the start barrier. Execute both rejection classes in both sealed workers. Do not invent an alignment rejection because `not_zero` is defined over a logical bit length with internal tail padding.
8. Add `benchmarks/stim_adapter/simd_bits_not_zero_contract.h` to the ordered adapter comparator receipt and bind its digest in every new group. Adapter receipt schema version 6 must reject omissions, reordering, substitutions, or altered bytes. Keep private Stab build-receipt schema version 2 but expand its framed source collection to `worker.rs`, `worker/bits.rs`, and `worker/not_zero.rs`. Contract-preflight schema version 5 must bind 42 actual receipts, and qualification report schema version 23 must preserve and replay that exact preflight plus the derived common-batch mode.
9. Keep one prepared vector live during timing and record setup and peak process RSS separately at all scales. Under `count-allocations`, prove zero timed allocations for every pattern at every runtime scale and the accepted maximum width. This slice makes no accepted cross-scale RSS ratio or Stim allocation claim; PQ6 remains the owner of resource-growth acceptance.
10. Apply independent `1.25x` median and bootstrap confidence-interval-upper thresholds to `not-zero` at every scale for all three groups. No aggregate, waiver, favorable-pattern substitution, or row median may hide a failure. A failed or noisy group stays implemented but unqualified and gains a source-owned profiler note with the observed cost and next owner action.
11. Add an exact replacement mapping from legacy pair `simd_bits_not_zero_100K` / `stab_simd_bits_not_zero_10K` only to the early group. Keep the legacy M12 submeasurement threshold active until clean completion evidence and migration review explicitly retire it. The clean early-hit completion receipt at revision `817d0fe870fd1b02c8e30f18e534e35df705a1ee` satisfied the migration gate, so the source-current M12 file retires only the `not_zero` pair and removes its stale replacement marker. Clean post-migration revision `60b732c77f1828058fbd65ec6c5c4ad582467fd1` then regenerated and replayed all three completion receipts at the source-current inventory. The all-zero and late groups remain additional regression protection, not alternative targets for the legacy pair.
12. Preserve one identical iteration count and equal work for every Stim/Stab pair. Standard common batches keep both implementations between 250 milliseconds and 2 seconds. If the independently calibrated early-hit speed ratio has no standard overlap, derive wide-ratio mode only when the implementation that selected fewer iterations is the sole side above 2 seconds, the common-iteration owner remains at or below 2 seconds, both remain at least 250 milliseconds, and neither exceeds 20 seconds. Report schema version 23 and offline replay must reject a fabricated mode or any floor, cap, equal-iteration, wrong-owner, or both-over-standard violation.
13. From one clean committed revision, run both CQ owners, private-worker reproducibility, each exact adapter probe, full and soak reports at all nine group-scale combinations, immediate report replay and regression checks, separate full and soak rollups for each group, and one completion receipt plus replay per group. Every receipt must bind one exact CPU identity and the same clean source revision. Finish with milestone audit and independent GPT-5.6/max full code review, fix every confirmed issue, record only genuine newly revealed under-specification, and publish a progress report with exact ratios and remaining architecture scope.

The accepted sixth-slice evidence inventory is `0161ab09015487ee2a1298be8dafe7c744b426b28a4e9fbdbd688e775c1655a0`. Clean revision `60b732c77f1828058fbd65ec6c5c4ad582467fd1` binds both exact CQ prerequisites, reproducible sealed workers, 42 actual contract-preflight probes per report, 18 passing first-attempt full and soak measurements, 18 successful report and regression replays, six replayed rollups, and three replayed completion receipts on controlled Linux AArch64. Median ratios range from `0.032329x` to `0.663712x`; the worst confidence-interval upper bounds are `0.071534x` for early-hit, `0.663577x` for all-zero, and `0.664097x` for late-hit. Accepted-maximum allocation instrumentation passes for every pattern. Final milestone re-audit and independent GPT-5.6/max review report no remaining finding or new under-specification. Native Linux x86-64, PQ6 memory growth, and every remaining PQ2 group remain unclaimed.

### Seventh Executable Slice

1. Graduate two independent execute-phase contracts from `src/stim/mem/sparse_xor_vec.perf.cc`: `PERFQ-M5-SPARSE-XOR` owns only the 1,000-row symmetric-difference loop, and `PERFQ-M5-SPARSE-XOR-ITEM` owns only the seven-item toggle loop. Never aggregate their timing, confidence interval, allocation result, or completion outcome because row merging and sorted insertion or removal are different algorithms.
2. Bind both groups exactly to `cq-evidence-qualification-bea77c19e9ae0b24`, whose independently selectable CQ2 case owns `SparseXorVec::xor_assign`, `SparseXorVec::xor_item`, sorted-unique invariants, duplicate cancellation, stack-to-heap transitions, and dense-reference equivalence. The exact case must pass from the same clean revision and current correctness digest before either ratio is promotable.
3. Reproduce the upstream row fixture as 1,000 rows containing `k`, `k+1`, `k+4`, `k+8`, and `k+15`. One complete callback performs 999 forward row XORs and 998 reverse row XORs, for 1,997 actual operations; preserve Stim's nominal `n * 2` rate only as provenance and use 1,997 as the qualification work denominator. Use `small`, `medium`, and `large` scales of 1, 64, and 4,096 complete callbacks, corresponding to 1,997, 127,808, and 8,179,712 row XORs.
4. Reproduce the upstream item fixture as the exact sequence `2, 5, 9, 5, 3, 6, 10`. Use `small`, `medium`, and `large` scales of 1, 64, and 4,096 complete callbacks, corresponding to 7, 448, and 28,672 item toggles. A scale must contain only complete seven-item callbacks so it cannot substitute a favorable insertion or removal subset.
5. Canonically encode the row input as a little-endian `u64` row count followed by a little-endian `u64` length and little-endian `u32` items for every row. Freeze 28,008 bytes and input digest `9fdcaf10b6a6437d51afade0e21f39acdd1130ff18255e38c0751261f93df2a2` at all row scales. Encode the item input as a little-endian `u64` sequence length followed by its seven little-endian `u32` items, and freeze 36 bytes and digest `c2c1749b4bf4c7c355c1d0a8109ea53bba790034d116acea3755b533c1fb1059` at all item scales.
6. Prepare each mutable fixture outside timing and execute two untimed complete callbacks to return it to its canonical initial state while retaining steady-state capacity. Time only complete source callbacks behind matching compiler fences and optimizer-opaque mutable references. Keep fixture creation, capacity priming, input hashing, final-state encoding, and output hashing outside timing. Count actual row XORs or item toggles, not fixture bytes or nominal upstream rates.
7. Encode each semantic output from exactly 12 little-endian `u64` fields: iteration count, declared work items, workload marker, base callback work, all four input-digest lanes, and all four final-state-digest lanes. Canonically encode final sparse state with the same length-delimited format as input. Freeze one-callback odd-state, two-iteration even-state, and accepted-maximum outputs in both worker test suites, and reject any input, work-count, or output disagreement before timing.
8. Accept only positive complete-callback work counts through the `large` scale. Reject 1,998 and 8,181,709 row work items and reject 8 and 28,679 item work items before allocation and before consuming the start barrier. Under `count-allocations`, prove zero timed Stab allocations for both workloads at every runtime scale and the accepted maximum after the exact two-callback capacity priming. Setup and peak process RSS remain report-only until PQ6 defines a cross-scale rule.
9. Add `benchmarks/stim_adapter/sparse_xor_contract.h` to the ordered adapter comparator receipt and bind its digest independently in both groups. Adapter receipt schema version 7 must reject omission, reordering, substitution, or altered bytes. Expand private Stab worker source collection to the new sparse-XOR module, bump contract-preflight schema to version 6 with 58 actual probes, and bump the qualification report schema so offline replay rejects prior worker or preflight identities.
10. Apply independent `1.25x` median and bootstrap confidence-interval-upper thresholds to `row-xor` and `xor-item` at every scale with no waiver path. A failed or noisy result remains visible with a source-owned profiler note, owner, and next action; neither group can close or migrate the other group's legacy pair.
11. Add exact replacement mappings from `SparseXorTable_SmallRowXor_1000` / `stab_sparse_table_row_xor_1000` to `PERFQ-M5-SPARSE-XOR/row-xor` and from `SparseXorVec_XorItem` / `stab_sparse_xor_item_7` to `PERFQ-M5-SPARSE-XOR-ITEM/xor-item`. Keep both legacy M12 submeasurement thresholds active through the first clean completion receipts and migration review. Clean revision `e2f6292f473b034d8886fc100039c7a78c4a3989` passed and replayed both first-stage completion receipts at pre-migration inventory `2d9cb3e3e2dc36a29c31964480f9b735e2411b26f4ba2b3ac66ed6791b617dc0`, authorizing retirement of exactly those legacy timing pairs and their replacement markers in one focused migration commit. Regenerate and replay the complete seventh-slice chain at the post-migration inventory before claiming source-current closure. Keep the M12 memory baseline until PQ6 supplies equal or stronger memory evidence.
12. From each clean evidence revision, run the exact CQ prerequisite, private-worker reproducibility, both adapter probes, full and soak reports at all six group-scale combinations, immediate report replay and regression, separate full and soak rollups for each group, and one completion receipt plus replay per group. The final post-migration chain therefore contains 12 promotable reports, 12 report replays, 12 regressions, four rollup replays, and two completion replays from one commit, worker identity, CPU identity, and current inventory.
13. Finish with milestone audit and independent GPT-5.6/max full code review over the Rust and C++ loops, canonical encodings, optimizer barriers, allocation claims, caps, receipt schemas, replacement migration, evidence, and documentation. Fix every confirmed defect, record only genuinely new under-specification, and publish `docs/plans/pq2-sparse-xor-qualification-progress-report.md` with exact hashes, ratios, confidence bounds, artifact paths, review closure, and remaining x86-64 and PQ6 scope.

The seventh slice deliberately amplifies complete pinned-Stim callbacks instead of inventing larger sparse-set shapes. Broader active-cardinality and density-crossover timing remains owned by the planned public-API groups, while exact density-transition correctness is already a prerequisite of these two contracts.

The accepted seventh-slice performance inventory is `8cc3ab3eb88faaf539c3c0eabaf3865ad421d8f67b14549cb4c7acc71faf2406`, with correctness inventory `ccb80eb4b660a375b59460c3b7fa03a810abd6f868735b566735378105db22b2`. Clean post-migration revision `7b43b46d1c08f669264d009b8d3872ce86838f0e` binds the exact CQ prerequisite, reproducible sealed workers, both adapter probes, 12 passing first-attempt full and soak reports, 12 report replays, 12 regression passes, four rollup replays, and two completion receipt replays on controlled Linux AArch64. Median ratios range from `0.965755x` to `1.026014x`, with worst confidence-interval upper bound `1.034133x`. The first-stage receipts at `e2f6292f473b034d8886fc100039c7a78c4a3989` authorized retirement of exactly the two duplicate legacy sparse-XOR timing pairs; the post-migration chain proves the replacement at its accepted inventory while preserving the M12 memory baseline. Milestone audit and closure verification found and closed missing public probe registration, stale documentation, and a stale threshold-row-count assertion, with no new under-specification. Independent GPT-5.6/max review found and closed the adapter-local schema, receipt-count, probe-command, and workload documentation drift and reported no code, comparator, resource, evidence, migration, or architecture finding. Later transpose ownership changed the source-current inventories without relabeling this evidence. Native Linux x86-64, PQ6 memory growth, broader sparse-density groups, and all remaining PQ2 groups remain unclaimed.

### Eighth Executable Slice

1. Graduate exactly two independent execute-phase contracts: `PERFQ-M5-BIT-MATRIX-TRANSPOSE-IN-PLACE` owns public `BitMatrix::transpose_square_in_place`, and `PERFQ-M5-BIT-MATRIX-TRANSPOSE-ALLOCATING` owns public `BitMatrix::transpose`. Assign both the root re-export and `stab_core::bits` rustdoc paths to their exact group, remove those four paths from the broad planned BitMatrix API parents, and leave every constructor, accessor, row mutation, clone, equality, and row-kernel path planned. Never aggregate the two transpose timings, allocation outcomes, confidence intervals, or completion receipts.
2. Bind both groups exactly to `cq-evidence-qualification-4d0291febfd22b68` for transpose semantics and `cq-evidence-qualification-66e29faafe5f2856` for checked matrix construction, row storage, and materialized resource boundaries. Both cases must pass from the same clean current-digest correctness report before either ratio is promotable.
3. Use a pinned-Stim adapter instead of the existing aggregate `stim_perf` row because qualification requires one deterministic nonzero fixture, exact semantic output, sealed worker symmetry, and separate public API allocation obligations. The in-place comparator must call `simd_bit_table<MAX_BITWORD_WIDTH>::do_square_transpose()`. The allocating comparator must call `simd_bit_table<MAX_BITWORD_WIDTH>::transposed()`, not the preallocated `transpose_into(out)` perf callback, so both implementations allocate and return one result per public call. Keep `simd_bit_table_inplace_square_transpose_diam10K` and `simd_bit_table_out_of_place_transpose_diam10K` as exact provenance without claiming that their zero-filled fixture or preallocated output is the final public-API comparator.
4. Use square dimensions 256, 2,048, and 16,384 for `small`, `medium`, and `large`. These dimensions are multiples of 256, so Stim's architecture-dependent 64-, 128-, or 256-bit table padding performs exactly the declared logical work on both Linux AArch64 and Linux x86-64. The scales contain 65,536, 4,194,304, and 268,435,456 transposed bits and 8,192, 524,288, and 33,554,432 logical data bytes before the canonical dimension header.
5. Define one reviewable non-symmetric fixture generator from a frozen `u64` seed. For every row, set a bounded fixed number of columns derived from checked affine and SplitMix64 transforms of the row and dimension; duplicate generated columns collapse to one set bit. Use the same assignment semantics and little-endian row-major logical words in both workers. Canonically encode `u64` row count, `u64` column count, then every logical row word, producing 8,208, 524,304, and 33,554,448 input bytes. Freeze every scale's input digest plus small odd, small even, and accepted-maximum output digests in both worker test suites before runtime registration; no digest may be copied from one worker without an independent recomputation test.
6. For in-place transpose, prepare one matrix outside timing, execute exactly two untimed complete transposes to restore canonical state and warm the code path, then time only repeated public in-place transposes behind matching compiler fences and optimizer-opaque mutable references. Encode semantic output from iteration count, declared work, dimension, workload marker, all four input-digest lanes, and all four final-state-digest lanes. Odd and even iteration counts must produce distinct frozen receipts, and the timed Stab body must make zero allocation calls and allocate zero bytes at every scale and the accepted maximum.
7. For allocating transpose, keep one immutable source matrix outside timing and execute and discard exactly two untimed public allocating transposes. Each timed iteration must call the public allocating API, pass the returned matrix through an optimizer-opaque sink, and drop the preceding result inside the timed body so allocation and destruction remain measured symmetrically. Keep the final result for untimed encoding. Encode iteration count, declared work, dimension, workload marker, all four input lanes, all four result lanes, and all four unchanged-source lanes. Stab allocation instrumentation must prove exactly one output-data allocation of `dimension * dimension / 8` requested bytes per single call at every scale and the accepted maximum; make no cross-implementation allocation-count claim without Stim allocator instrumentation.
8. Implement a safe blocked transpose kernel behind `BitMatrix` instead of benchmarking the existing per-bit `get` and `set` loops as the final design. Put the 64-by-64 tile primitive, edge handling, and any direct `std::simd` use in a dedicated bit-kernel module with a scalar reference. Out-of-place transpose must allocate the checked target shape once and fill it by tiles. Square in-place transpose must transpose diagonal tiles and exchange-transpose off-diagonal tile pairs using bounded stack scratch without heap allocation. Preserve zero dimensions, rectangular allocating transpose, unaligned logical edges, canonical tail masking, checked overflow, and rectangular in-place rejection; do not add architecture-specific intrinsics or a new public raw-word or `transpose_into` API merely to ease the benchmark.
9. Add focused kernel and public-API tests before optimization: exhaustive small matrices, deterministic randomized matrices, 63/64/65 and 255/256/257 boundaries, rectangular shapes in both orientations, double transpose, in-place versus allocating equality, dirty-tail isolation, source immutability, rectangular in-place rejection before mutation, matrix-size overflow, and scalar-reference equivalence. Add an allocation test that fails on the current allocating in-place implementation and passes only when in-place transpose is heap-allocation-free; separately enforce the one-allocation public allocating contract.
10. Accept only perfect-square work counts whose derived dimension is a positive multiple of 256 from 256 through 16,384. Validate semantic work as checked `iterations * work_items` before fixture allocation and before consuming the start barrier, then validate the integer square root, dimension alignment, checked byte count, and cap. In both sealed workers and both workload ids, accept the three runtime scales and reject 65,025 bits for a below-minimum 255-square, 65,537 bits for a non-square, 66,049 bits for an unaligned 257-square, 276,889,600 bits for the first aligned over-cap 16,640-square, and a valid 256-square paired with `2^48` iterations whose semantic work overflows `u64`. The overflow rejection must run with the start barrier enabled so consuming the barrier or allocating the fixture first deadlocks or otherwise fails the preflight instead of passing unnoticed.
11. Add `benchmarks/stim_adapter/bit_matrix_transpose_contract.h` to the ordered comparator receipt. Adapter receipt schema version 8 must reject omission, reordering, substitution, or altered bytes. Add an isolated Rust transpose worker module to the private source receipt, bump contract-preflight schema to version 8 with 90 actual accepted or rejected receipts, and bump qualification report schema to version 26 so offline replay rejects every prior worker, comparator, preflight, or report identity. Add distinct `pq2-bit-matrix-transpose-in-place-adapter-smoke` and `pq2-bit-matrix-transpose-allocating-adapter-smoke` probes with source-owned defaults and complete bound validation.
12. Keep production and validation modules below 1,200 lines. Before adding transpose dispatch, extract an explicit owner from `runtime/worker.rs`, `runtime/invocation.rs`, `runtime/adapter.rs`, or `qualification/validation.rs` when the touched file would cross the limit. Include every extracted executable source in the appropriate build or comparator fingerprint, and add replay tests that fail when any source is omitted or reordered.
13. Apply independent `1.25x` median and bootstrap confidence-interval-upper thresholds to `in-place-transpose` and `allocating-transpose` at every scale with no waiver path. Use diagnostic probes and profiler evidence before promotable runs. If either group is slow or noisy, retain the first result, add `benchmarks/profiler-notes/pq2-bit-matrix-transpose-<kind>.md` with the dominant cost and next owner action, optimize the real public hot path without weakening work or output obligations, and restart clean evidence only after committing the changed implementation. Never rerun a non-noisy formal result toward a favorable sample.
14. Keep the heterogeneous legacy `m5-simd-bit-table` row-level `1.25x` timing threshold active through both first-stage completion receipts. Its 128-by-128 Stab transpose proxy is not a faithful replacement for either new scale family, and its row-XOR measurement is unrelated; do not invent measurement pairings. Extend the source-owned migration ledger if necessary so review can bind both pinned Stim transpose symbols and the obsolete Stab proxy disposition to the two exact runtime groups without duplicate ownership. Only after both clean completion receipts pass may one focused migration commit supersede the legacy timing row and remove its row-level timing threshold. Preserve `benchmarks/m12-primary-memory-baseline.json` until PQ6 supplies equal or stronger memory evidence.
15. From each clean evidence revision, run both exact CQ prerequisites, private-worker reproducibility, both adapter probes, full and soak reports for both groups at all three scales, immediate report replay and regression, separate full and soak rollups for each group, and one completion receipt plus replay per group. If migration is authorized, regenerate and replay that complete 12-report, 12-regression, four-rollup, and two-completion chain from one clean post-migration revision and current inventory before claiming closure.
16. Finish with milestone audit and independent GPT-5.6/max full code review over API ownership, scalar and blocked kernels, portable-SIMD isolation, edge semantics, Rust and C++ callback fidelity, canonical encodings, optimizer barriers, allocation claims, hostile bounds, receipt schemas, legacy migration, evidence, and documentation. Fix every confirmed defect, log only genuinely new under-specification, and publish `docs/plans/pq2-bit-matrix-transpose-qualification-progress-report.md` with exact hashes, ratios, confidence bounds, allocation outcomes, artifact paths, audit and review closure, and remaining x86-64 and PQ6 scope.

The eighth slice deliberately does not qualify row XOR, masked row XOR, row swaps, construction, parsing, serialization, matrix multiplication, inversion, or raw random-fill APIs. It splits only the two public transpose methods from their broad planned API parents and leaves every other exact path visible for later groups.

The accepted eighth-slice performance inventory is `1d38c155acbaf78234f9b92857cfef8c25ffa059a4a9e9756b272a72272dfd0d`, with correctness inventory `5d795e831bc20b3f2780ca72c1eaea7c75387388d38f8e37f4539254a41e821b`. Clean reviewed revision `f912cc3af1f13cc9fab798d69937c155d37d83a0` binds both exact CQ prerequisites, reproducible sealed workers, both adapter probes, 12 passing first-attempt full and soak reports, 12 report replays, 12 regression passes, four method-specific rollup replays, and two method-specific completion receipt replays on controlled Linux AArch64. In-place median ratios range from `0.366307x` to `0.684489x`, with worst confidence-interval upper bound `0.686065x`; allocating medians range from `0.397088x` to `0.657658x`, with worst upper bound `0.660876x`. Allocation instrumentation proves zero timed in-place allocations and exactly one correctly sized public output allocation for allocating transpose at every runtime scale and accepted maximum. The earlier first-stage receipts authorized retirement of only the heterogeneous legacy timing threshold, while the M12 memory baseline remains for PQ6. Milestone audit found no new under-specification, and the final independent GPT-5.6/max review reported no P0 through P3 finding and no actionable defect. Exact hashes and closure evidence are recorded in `docs/plans/pq2-bit-matrix-transpose-qualification-progress-report.md`. Native Linux x86-64, PQ6 memory growth, and every remaining PQ2 group remain unclaimed; the separate 271-parent CQ2 checkpoint is source-current at clean hardened-controller revision `3f2f382627c8421de0a668819d467a9f252de20f`.

Ninth-slice migration and closure note, 2026-07-17: correctness digest `3db44922e3310cb3a573fcff3b28d5eea5d28e0d6975e0856965c601ecc23c72` and pre-migration performance digest `84d5ab682acda2a847972a74c5d58443fde8d2c820e62e46b634562e7c918e46` produced six first-attempt passing reports, six report replays and regressions, two rollups, and completion receipt `7439bf55e72a3c1e1b9cbf9b0648d525b5ad9eaf05718386122c6a3cafd50522` plus replay from clean revision `3a0fcd814f8d1a9441420ab85edf3d757572ba93`. Median ratios ranged from `1.002254x` to `1.031540x`, with worst upper bound `1.031846x`. This authorized migration commit `42c132f2c49538364649cd90962166223c72b4c6`; intermediate post-migration performance digest `7eedf59cb65d2bd244accc56973d7831001191cd62511c56b05a5cd7ed612ac6` superseded the identity-only timing row and retained its memory baseline. Strengthening the exact CQ owner then set correctness digest `a739d350eeb3455d4a0b386f8a257d3d4fe01d417d7d11d8a269229d68a6a103` and dependent performance digest `e79edf2e1eaa49a801606245d4a845d47a1d000ed527c9669d95e091c4480237`. Clean reviewed revision `cd1e33e10f45995ccaca498547ff5aa88bfe51bb` regenerated and replayed the exact correctness preflight, worker identity, adapter probe, six first-attempt reports and regressions, two rollups, and completion receipt at those then-current inventories. Its median ratios range from `1.001956x` to `1.032352x`, with worst upper bound `1.032881x`, and exact artifacts are recorded in `docs/plans/pq2-pauli-string-multiplication-qualification-progress-report.md`.

### Ninth Executable Slice

1. Graduate exactly one execute-phase contract, `PERFQ-M6-PAULI-STRING`, for public `PauliString::right_multiply_in_place_returning_log_i_scalar`. Assign only `stab_core::PauliString::right_multiply_in_place_returning_log_i_scalar` and `stab_core::stabilizers::PauliString::right_multiply_in_place_returning_log_i_scalar` to this exact group. Leave allocating multiplication, real-only multiplication, scalar-product queries, commutation, construction, parsing, formatting, randomization, accessors, trait implementations, FlexPauliString, CliffordString, iterators, Tableau, and every other Algebra path in their own planned parents.
2. Bind the group exactly to `cq-evidence-qualification-3bab0f51237445f6` for multiplication bases and phases and `cq-evidence-qualification-489e6445120743c2` for the Pauli materialization boundary. Strengthen the first case before timing so an independent per-qubit scalar oracle checks the direct in-place result and returned base-`i` exponent at 1, 63, 64, 65, 10,000, 100,000, and 1,000,000 qubits, checks odd and even repeated calls, preserves the right operand and left real sign, and cannot pass through comparison to allocating `multiply`, which shares the same in-place core.
3. Use a pinned-Stim adapter because the inherited `PauliString_multiplication_10K`, `PauliString_multiplication_100K`, and `PauliString_multiplication_1M` perf callbacks construct two identity operands. Stab's public method intentionally returns early for an identity right operand, so the inherited row is provenance and a temporary legacy gate, not faithful evidence of full-width mutation. The comparator must call `PauliString<MAX_BITWORD_WIDTH>::ref().inplace_right_mul_returning_log_i_scalar`, and the Rust worker must call the exact public Stab method with equal-width operands.
4. Use logical widths 10,000, 100,000, and 1,000,000 as `small`, `medium`, and `large`, preserving the three upstream source-symbol sizes. Semantic work is the checked product of timed public calls and logical qubits. The accepted benchmark-worker maximum is the public `StabilizerResource::PauliQubits` limit of 1,048,576; the runtime ledger still exposes only the three named scales and rejects caller-selected replacements.
5. Define one deterministic dense non-identity fixture named `pauli-right-multiply-splitmix64-v1` with numeric generator and workload marker `5`. At logical qubit `q`, derive the left basis from the low two bits of SplitMix64 applied to `0x243f6a8885a308d3 + q * 0x9e3779b97f4a7c15` and derive the right basis from the low two bits of SplitMix64 applied to `0x13198a2e03707344 + q * 0xbf58476d1ce4e5b9`, with wrapping `u64` arithmetic. Use a positive left sign encoded as `0` and negative right sign encoded as `1`. Encode four little-endian `u64` header fields for width, generator marker, left sign, and right sign, followed by complete logical `left_x`, `left_z`, `right_x`, and `right_z` word planes with canonical tail masking. The exact input byte counts are 5,056, 50,048, and 500,032 for the runtime scales and 524,320 at the accepted maximum. Freeze independently recomputed input and odd/even output digests before inventory graduation.
6. Prepare both operands outside timing, execute exactly two untimed complete public multiplications to restore the canonical left operand and warm the code path, then time only repeated direct in-place calls behind matching compiler fences and optimizer-opaque references. Accumulate every returned base-`i` exponent in the timed loop and retain the final left operand. Canonical output must contain exactly 17 little-endian `u64` fields for iteration count, declared semantic work, width, workload marker, phase checksum, four input-digest lanes, four final-left-digest lanes, and four unchanged-right-digest lanes. Freeze distinct odd and even receipts.
7. Isolate the complete-word Pauli multiplication primitive behind `stab-core`'s private bit-kernel boundary. Keep an independent scalar reference, put any direct `std::simd` use only in `crates/stab-core/src/bits/simd.rs`, preserve scalar tails and canonical tail bits, and do not add architecture-specific intrinsics, `unsafe`, a public raw-word constructor, or a public preallocated multiplication API. Optimize only after the faithful diagnostic probe establishes the real ratio and profile.
8. Add focused tests before accepting optimization: exhaustive short basis pairs and signs, deterministic randomized 63/64/65 and 255/256/257 widths, scalar-reference equivalence, odd/even state restoration, negative-right phase contribution, unequal-length extension, identity fast-path preservation, right-operand immutability, and canonical tail behavior. Under the `count-allocations` feature, prove zero allocation calls and bytes for one equal-width timed public call at all three scales and 1,048,576 qubits; do not claim a Stim allocation count without separate instrumentation.
9. Make both sealed workers accept widths from 1 through 1,048,576 and reject zero, 1,048,577, malformed workload identity, and checked `iterations * width` overflow before fixture construction and before consuming an enabled start barrier. Freeze small odd, small even, and accepted-maximum successful receipts plus every rejection for both implementations. Keep setup and peak RSS report-only until PQ6 defines cross-scale memory acceptance.
10. Refactor before extending any production or validation source that would reach 1,200 lines. Put the Rust workload in an isolated fingerprinted worker module and the C++ fixture and callback in `benchmarks/stim_adapter/pauli_string_multiply_contract.h`; extract adapter, invocation, probe, or validation ownership where needed instead of growing near-limit dispatch files into mixed-purpose modules.
11. Extend legacy replacement contracts with an optional exact runtime scale id. Validate that a present scale belongs to the target runtime group, include it in replacement-target uniqueness, generated inventory, runtime-contract validation, and adversarial tests, and preserve compatibility for an existing scale-family mapping that intentionally omits the field. Before migration, map `PauliString_multiplication_10K` / `stab_pauli_string_multiplication_10K` to `small`, the 100K pair to `medium`, and the 1M pair to `large`, all targeting `PERFQ-M6-PAULI-STRING/right-multiply-in-place`.
12. Add `benchmarks/stim_adapter/pauli_string_multiply_contract.h` to the ordered comparator receipt and bump adapter receipt schema to version 9. Include the isolated Rust worker in the private source receipt, bump contract-preflight schema to version 9 with the exact accepted and rejected receipt count produced by the reviewed matrix, and bump qualification report schema to version 27. Add `pq2-pauli-string-multiply-adapter-smoke` with source-owned defaults and complete bound validation; replay must reject omitted, reordered, substituted, or altered worker and comparator sources and every stale schema.
13. Apply an independent `1.25x` median and bootstrap confidence-interval-upper threshold to `right-multiply-in-place` at every scale with no waiver path. Retain the first diagnostic and formal outcome. If the group is slow or noisy, add `benchmarks/profiler-notes/qualification/perfq-m6-pauli-string.md` with the dominant cost and owner action, optimize the real public path without weakening fixture, work, output, allocation, or cap obligations, commit the changed implementation, and restart promotable evidence from that clean revision. Never rerun a non-noisy formal result toward a favorable sample.
14. Keep the legacy `m6-pauli-string` row-level `1.25x` timing threshold and explicit `1.25x` thresholds for each 10K, 100K, and 1M measurement pair active through the first clean completion receipt. The old identity-only callback and Stab early return are not the new non-identity comparison, so do not cite any legacy ratio as authorization. Once the pre-migration receipt passes and replays with all three scale-aware source mappings, retire only those legacy timing thresholds and mappings in one focused migration commit. Preserve `benchmarks/m12-primary-memory-baseline.json` until PQ6 supplies equal or stronger memory evidence.
15. From the clean post-migration revision, rerun the exact CQ prerequisites, private-worker reproducibility, adapter probe, full and soak reports at all three scales, immediate report replay and regression, separate full and soak rollups, and one completion receipt plus replay. Claim closure only from that complete six-report, six-regression, two-rollup, one-completion source-current chain; earlier diagnostics and pre-migration authorization remain historical under their exact inventories and worker identities.
16. Finish with milestone audit and independent GPT-5.6/max full code review over exact API ownership, scalar and portable-SIMD kernels, fixture independence, phase math, odd/even state, Rust and C++ callback fidelity, optimizer barriers, allocation claims, hostile bounds, scale-aware migration, receipt schemas, evidence, and documentation. Fix every confirmed defect, log only genuinely new under-specification, and publish `docs/plans/pq2-pauli-string-multiplication-qualification-progress-report.md` with exact hashes, ratios, confidence bounds, allocation outcomes, artifact paths, audit and review closure, and remaining x86-64 and PQ6 scope.

The ninth slice deliberately qualifies only equal-width direct in-place Pauli multiplication with a returned scalar phase. It does not qualify allocating multiplication, unequal-width growth performance, identity-fast-path timing, commutation, randomization, iteration, Clifford multiplication, Tableau operations, or the rest of Algebra.

The accepted ninth-slice performance inventory is `e79edf2e1eaa49a801606245d4a845d47a1d000ed527c9669d95e091c4480237`, with correctness inventory `a739d350eeb3455d4a0b386f8a257d3d4fe01d417d7d11d8a269229d68a6a103`. Clean reviewed revision `cd1e33e10f45995ccaca498547ff5aa88bfe51bb` binds the strengthened exact two-case correctness prerequisite, reproducible sealed workers, adapter probe, six passing first-attempt full and soak reports, six report replays, six regression passes, two rollup replays, and one completion receipt replay on controlled Linux AArch64. Median ratios range from `1.001956x` to `1.032352x`, with worst confidence-interval upper bound `1.032881x`. Allocation instrumentation proves zero timed Stab allocation calls and bytes at every runtime scale and the accepted maximum. The pre-migration receipt authorized retirement of only the identity-fast-path legacy timing row and its exact pair thresholds, while the M12 memory baseline remains guarded for PQ6. Milestone audit and independent GPT-5.6/max follow-up review report no remaining P0 through P3 finding and no new under-specification. Exact hashes and closure evidence are recorded in `docs/plans/pq2-pauli-string-multiplication-qualification-progress-report.md`. Native Linux x86-64, PQ6 memory growth, and every remaining PQ2 group remain unclaimed; the separate 271-parent CQ2 checkpoint is source-current at clean hardened-controller revision `3f2f382627c8421de0a668819d467a9f252de20f`.

### Tenth Executable Slice

1. Replace the bundled inherited `m6-pauli-iter` timing claim with two independent execute-phase contracts. `PERFQ-M6-PAULI-ITER` owns X/Z enumeration over weights 2 through 5, and `PERFQ-M6-PAULI-ITER-SINGLETON` owns X/Y/Z enumeration at exactly weight 1. Both measurements are named `construct-and-iterate-borrowed`. Never aggregate their timing, confidence interval, allocation result, rollup, or completion outcome because combinatorial weight-range advancement and wide singleton position advancement execute different work.
2. Assign only root and module-reexport paths for `PauliStringIterator::new`, `PauliStringIterator::iter_next`, and `PauliStringIterator::result` to `PERFQ-M6-PAULI-ITER`. Keep `restart`, owned `Iterator::next`, clone, formatting, comparison traits, `CommutingPauliStringIterator`, and all other iterator and Algebra paths in their existing planned parents. Treat the singleton group as required workload-shape protection for the same borrowed public lifecycle rather than a second owner of the same API ids.
3. Bind both groups to the exact source owners `cq2-algebra-pauli-iterator-order-contract`, `cq2-algebra-pauli-iterator-api-contract`, and `cq2-algebra-resource-pauli-materialization` after regenerating their current hashed ids. Move the strengthened order owner to a dedicated integration-test target before adding cases because `crates/stab-core/tests/stabilizers.rs` is already near the 1,200-line watch limit.
4. Strengthen the exact order owner before timing with an independent sparse reference enumerator. Cover exhaustive small axis and weight combinations, borrowed result reuse, deterministic first and last values, exact output count, exact total result-width checksum, and a stable sparse sequence digest. Exercise X/Z range widths 5, 11, and 22; X/Y/Z singleton widths 1,000, 32,000, and 1,000,000; 63, 64, 65, 255, 256, and 257 word boundaries; restart equivalence; complete all-output traversal with full yielded-state validation at the 1,048,576-qubit accepted boundary; and the typed 1,048,577-qubit rejection without using the owned `Iterator::next` path as the oracle.
5. Preserve the exact pinned-Stim lifecycle from `src/stim/stabilizers/pauli_string_iter.perf.cc`: every timed callback constructs one iterator, repeatedly calls borrowed `iter_next`, consumes the borrowed result width and output count, and destroys the iterator before returning. The Rust worker must call public `PauliStringIterator::new`, `iter_next`, and `result`; the C++ worker must call `PauliStringIterator<MAX_BITWORD_WIDTH>` and its public `iter_next` and `result` field. Do not reuse iterator state across callbacks, time `restart`, clone owned results, format values, or replace traversal with a combinatorial count.
6. Use output count as semantic work per timed callback because it is the exact source-owned `PauliStrings` rate denominator. For X/Z range enumeration, freeze width 5 with 232 outputs as `small`, width 11 with 21,604 outputs as `medium`, and width 22 with 972,972 outputs as `large`. Width 22 is also the accepted benchmark-worker maximum under a 1,000,000-output combinatorial cap; width 23 with 1,233,628 outputs is the first rejected range fixture.
7. For X/Y/Z singleton enumeration, freeze width 1,000 with 3,000 outputs as `small`, width 32,000 with 96,000 outputs as `medium`, and width 1,000,000 with 3,000,000 outputs as `large`. The worker accepts widths through the public 1,048,576-qubit limit, corresponding to 3,145,728 outputs, and rejects width 1,048,577 before allocation or barrier consumption.
8. Represent each scale's canonical input as exactly eight little-endian `u64` fields for width, minimum weight, maximum weight, allowed-axis mask, outputs per traversal, workload marker, benchmark output cap, and public qubit cap. Use distinct workload markers for range and singleton enumeration. Freeze independently reproduced input digests for all six runtime scales and both accepted maxima before inventory graduation.
9. Produce an output digest from exactly eighteen little-endian `u64` fields: iteration count, checked semantic work, width, workload marker, minimum weight, maximum weight, allowed-axis mask, outputs per traversal, observed output count, observed total result-width checksum, four canonical-input digest lanes, and four final-result digest lanes from one untimed validation traversal. Both workers must verify the untimed traversal count, width checksum, and final result before entering timing, then black-box the timed observed counters and encode the digest outside timing.
10. Reject malformed output counts, invalid range counts, zero work, over-cap range width, over-cap singleton width, wrong measurement id, checked `iterations * outputs` overflow, and checked `iterations * outputs * width` checksum overflow before fixture allocation and before consuming an enabled start barrier. Freeze accepted small odd, small even, accepted-maximum, malformed-shape, first-over-cap, wrong-measurement, work-overflow, and width-checksum-overflow receipts for both implementations and both workload shapes.
11. Add allocation-counted Stab tests for one complete public callback at all six runtime scales and both accepted maxima. Record exact constructor, traversal-state, and result-storage allocation calls and requested bytes if stable; otherwise freeze a reviewed source-derived upper bound that cannot silently grow. Do not claim a Stim allocation count without independent allocator instrumentation, and keep setup and peak RSS report-only until PQ6 defines cross-scale memory acceptance.
12. Extract `WorkerWorkload` and its source-owned ids from `worker.rs` before adding the two variants so the central worker stays below 1,200 lines. Put iterator execution in `worker/pauli_iter.rs`, iterator invocation and output validation in `invocation/pauli_iter.rs`, and the independent pinned-Stim callback in `benchmarks/stim_adapter/pauli_string_iter_contract.h`. Add every new ordered source to the appropriate receipt instead of hiding code behind an unhashed include.
13. Bump the private Stab build-receipt schema, adapter receipt schema, contract-preflight schema, and qualification report schema when their ordered source or receipt contracts change. Replace every stale-schema test, freeze the exact accepted and rejected preflight receipt count produced by the reviewed matrix, and make replay reject omitted, reordered, substituted, or altered worker and comparator sources.
14. Add `pq2-pauli-iter-range-adapter-smoke` and `pq2-pauli-iter-singleton-adapter-smoke` with source-owned defaults and complete bound validation. Each probe must prove exact worker agreement, accepted maximum behavior, malformed shape rejection, first over-cap rejection, wrong measurement rejection, semantic-work overflow rejection, and width-checksum overflow rejection. Probe timings remain diagnostic and cannot become product ratios.
15. Apply independent `1.25x` median and bootstrap confidence-interval-upper thresholds to each group at every scale with no waiver path. Retain the first diagnostic and formal outcomes. If either group is slow or noisy, add a group-specific profiler note, profile the exact public lifecycle, optimize the implementation without changing fixture, work, output, allocation, or cap obligations, commit the change, and restart every promotable artifact for the affected group from that clean revision. Never rerun a non-noisy formal result toward a favorable sample.
16. Add exact replacement mappings from legacy pair `pauli_iter_xz_2_to_5_of_5` / `stab_pauli_iter_xz_2_to_5_of_5` to `PERFQ-M6-PAULI-ITER/construct-and-iterate-borrowed` at `small` and from `pauli_iter_xyz_1_of_1000` / `stab_pauli_iter_xyz_1_of_1000` to `PERFQ-M6-PAULI-ITER-SINGLETON/construct-and-iterate-borrowed` at `small`. Keep the bundled `m6-pauli-iter` M12 timing threshold active until both clean completion receipts pass and replay. Then retire only that legacy timing threshold and both mappings in one focused migration commit, mark the inherited row superseded, preserve `benchmarks/m12-primary-memory-baseline.json`, and explicitly guard the retained memory row until PQ6 replaces it.
17. From one clean pre-migration revision, run the exact three-case CQ prerequisite, private-worker reproducibility, both adapter probes, full and soak reports for both groups at all three scales, immediate report replay and regression, separate full and soak rollups for each group, and one completion receipt plus replay per group. Use those receipts only to authorize the focused timing migration. From one clean post-migration revision, regenerate and replay the same 12-report, 12-regression, four-rollup, and two-completion chain at the current inventories before claiming source-current closure.
18. Finish with milestone audit and independent GPT-5.6/max full code review over exact API ownership, independent reference enumeration, small and wide order, constructor-plus-traversal lifecycle, Rust and C++ comparator fidelity, optimizer barriers, allocation claims, hostile bounds, overflow timing, paired migration, receipt schemas, evidence, and documentation. Fix every confirmed defect, log only genuinely new under-specification, and publish `docs/plans/pq2-pauli-string-iterator-qualification-progress-report.md` with exact hashes, group-specific ratios, confidence bounds, allocation outcomes, artifact paths, audit and review closure, and remaining x86-64 and PQ6 scope.

The tenth slice deliberately qualifies only borrowed Pauli-string construction and complete traversal for the two pinned workload shapes. It does not qualify iterator restart timing, owned `Iterator::next`, cloning, formatting, comparison, arbitrary axis or weight distributions, commuting-Pauli iteration, Tableau iteration, or any other Algebra path.

Accepted tenth-slice checkpoint, 2026-07-18: correctness inventory `4c940e983df10a7c95cc512939f4a0cce79f1865e141739af9378db581ea5f87` and frozen post-migration performance inventory `48eacf03a2ecdca917c05aade52b7e17c9ead1be8b75b203e1d43c2f3b3b7dbf` contain both executable iterator contracts and their exact three-case prerequisites. Core revision `8503f458eac09fe94dba54cf8d6e16e88e195df4` provides the active-term iterator optimization, clean pre-migration revision `f2388dccc01abb7ef89e5f56d9062c6656837470` authorized retirement of only the bundled iterator timing threshold and both mappings, and migration commit `d706634eeaa536b2ce48d3dc9431b4feb513317f` made that focused change while preserving the memory baseline. Clean reviewed evidence revision `afaf0bf7f236b9f6ae6f72c19bbbdfea94d26632` publishes complete accepted-maximum correctness, reproducible workers, both complete adapter probes, 12 first-attempt passing reports and regressions, four rollups, and two completion receipts. Median ratios range from `0.025664x` to `0.568566x`, with worst confidence-interval upper bound `0.570628x`. The private Stab build receipt remains schema version 3, the adapter receipt and contract preflight remain schema version 10 with 140 exact accepted and rejected receipts, and qualification reports remain schema version 28. Milestone audit found and resolved the accepted-maximum specification loophole with no remaining implementation finding. Two final GPT-5.6/max review lanes found and closed operational-documentation and provisional-status inconsistencies, then reported no remaining P0 through P3 finding. Exact closure artifacts are recorded in `docs/plans/pq2-pauli-string-iterator-qualification-progress-report.md`. Current performance inventory `c238dc4e2500192f310ef3d2378ecaafc9744662b5127784dd4eeb6c60726176` preserves both iterator contracts and thresholds, retains the two Clifford-string contracts and the checked authorization for their legacy timing migration, and does not inherit any pre-migration Clifford ratio. The inherited iterator and Clifford rows remain superseded, and their memory baselines remain explicitly guarded for PQ6.

### Eleventh Executable Slice

Pre-migration checkpoint, 2026-07-19: clean revision `127d6661a9e00872fc4aa4c0b0d27171e005afa5` completed exact focused CQ, byte-reproducible workers, both adapter probes, all full and soak scale reports and regressions, four replayed rollups, and two replayed completion receipts under inventory `0ee3639389860799298164c94c647fcab45b03c9d67b941b1aad12c6e5e06df5`. Identity completion report SHA-256 `78fc10ca29e432641f3d978ed871c4b96d1ba344d714c20bf726f574239d2126` authorized retirement of only the inherited `m6-clifford-string` timing threshold and its exact identity/small mapping; non-identity completion report SHA-256 `f5842ddcf86f024a78293b203196e9490396ffb0762196a6f2cc169b1f8489c6` separately closes the companion pre-migration contract. The source-current inventory performs that narrow migration and preserves the M12 memory baseline; those pre-migration reports remain historical under their recorded inventory.

Historical pre-review eleventh-slice checkpoint, 2026-07-19: focused migration revision `91f62d0a78659da2e8e264a6968b3c6cd32456de` preserved the M12 memory baseline and published a complete schema-version-30 post-migration chain. Exact focused CQ, byte-reproducible workers, both adapter probes, all 12 first-attempt full and soak reports and regressions, four replayed rollups, and two replayed completion receipts passed on controlled Linux AArch64. Identity medians ranged from `0.000145x` to `0.014381x`, non-identity medians ranged from `0.742952x` to `0.766078x`, and the worst bootstrap confidence-interval upper bound was `0.766206x`. The subsequent independent review found that formal producers could replace an existing artifact, threshold retirement was not machine-bound, the worker lifecycle needed separation, and the hostile Clifford corpus omitted valid-marker, width/work, and malformed-hex adversaries. Those defects are fixed in the source-current schema-version-31 contract, which requires a fresh clean two-group chain before this checkpoint can be accepted as source-current. Exact historical artifacts and current closure status are recorded in `docs/plans/pq2-clifford-string-qualification-progress-report.md`.

1. Qualify Clifford-string in-place multiplication as two independent execute-phase groups. Existing `PERFQ-M6-CLIFFORD-STRING` becomes the exact pinned identity workload with measurement `right-multiply-identity`; add `PERFQ-M6-CLIFFORD-STRING-NON-IDENTITY` with measurement `right-multiply-non-identity`. Never use the identity result as evidence for the non-identity path: pinned `CliffordString_multiplication_10K` constructs two identity strings, while Stab's exact non-identity-count metadata makes that operation O(1).
2. Assign the inherited `m6-clifford-string` row only to the identity group. Assign root and module-reexport paths for public `CliffordString::right_multiply_in_place` only to the non-identity group. Keep allocating `multiply`, unequal-width growth, construction, clone, comparison, display, concatenation, repetition, randomization, indexing, and mutation in their existing planned groups because neither timed contract proves those phases.
3. Bind both groups to the current hashed forms of `cq2-algebra-clifford-string-api-contract`, `cq2-algebra-clifford-group-contract`, and `cq2-algebra-resource-clifford-growth`. Before timing, strengthen the first case to compare equal-width in-place output and right-operand immutability for deterministic identity and complete non-identity cycles, retain the independent all-24-by-24 Tableau-backed multiplication proof in the group case, and strengthen the resource case with accepted 1,048,576-qubit equal-width multiplication plus typed 1,048,577-qubit rejection.
4. Freeze the identity fixture as equal-width left and right identity strings. Freeze the non-identity fixture against pinned Stim's exact `all_cliffords_string` order: `I`, `X`, `Y`, `Z`, `H_XY`, `S`, `S_DAG`, `H_NXY`, `H`, `SQRT_Y_DAG`, `H_NXZ`, `SQRT_Y`, `H_YZ`, `H_NYZ`, `SQRT_X`, `SQRT_X_DAG`, `C_XYZ`, `C_XYNZ`, `C_NXYZ`, `C_XNYZ`, `C_ZYX`, `C_ZNYX`, `C_NZYX`, `C_ZYNX`. Assign canonical one-byte codes `0` through `23` in exactly that order. Position `i` uses left code `i % 24` and right code `1 + ((i / 24) % 23)`, so each complete 552-position cycle covers every left Clifford against every non-identity right Clifford exactly once. Repeat the cycle without RNG state, require the first 552 positions to remain a complete cross-product in every timing scale, and freeze tail lengths `64`, `88`, `328`, and `328` for widths 10,000, 100,000, 1,000,000, and 1,048,576. Their final pairs are respectively code pairs `(15,3)`, `(15,4)`, `(15,14)`, and `(15,14)`.
5. Use `small`, `medium`, and `large` widths of 10,000, 100,000, and 1,000,000 qubits for each group. The identity `small` scale exactly matches the pinned upstream rate denominator. Count checked `iterations * width` single-qubit products as semantic work for both groups even when Stab legitimately skips an identity-right callback through metadata.
6. Encode each canonical input descriptor as eight little-endian `u64` fields: width, workload marker, fixture schema, canonical gate count, right-cycle count, complete cross-product span, public Clifford-qubit cap, and reserved zero. The identity marker is the little-endian `u64` encoding of ASCII bytes `CLIF_ID1`, the non-identity marker is the little-endian `u64` encoding of `CLIF_NI1`, fixture schema is `1`, canonical gate count is `24`, and the cap is `1_048_576`. Identity uses right-cycle count and span `0`; non-identity uses `23` and `552`. Freeze the raw 64-byte descriptor and digest for all six runtime scales and both accepted maxima, and reject any field that differs from the exact selected workload contract.
7. Construct both equal-width operands from the canonical descriptor before the start barrier. After all untimed setup for one worker invocation and immediately before its start barrier, initialize `callback_count` and `execution_witness` to `0_u64`; calibration, validation, and warmup invocations use independent state and cannot contribute to a measured invocation. Every timed iteration in both workers must execute a sequentially consistent compiler fence, pass the mutable left reference and immutable right reference through a receipt-owned optimizer-opaque primitive, invoke public Rust `CliffordString::right_multiply_in_place` or pinned C++ `CliffordString<MAX_BITWORD_WIDTH>::operator*=` through those opaque references, and execute a matching compiler fence. After each successful call, increment `callback_count`, read the canonical code at opaque index `(callback_count - 1) % width` from the mutated left operand, and update `execution_witness = rotl64(execution_witness ^ code, 13) + 0x9e3779b97f4a7c15 + callback_count` modulo `2^64`; pass the observed code and updated witness through the opaque primitive. The identity workload therefore has literal one-call witness `0x9e3779b97f4a7c16` and two-call witness `0x8d6ea9a2cecd4fdd`. Leave the right operand unchanged and black-box the final left state after the loop as a second witness. Do not rely only on a post-call barrier, derive the witness from request fields without reading the result, reconstruct operands inside timing, substitute allocating multiplication, call a private kernel directly, carry witness state across invocations, or replace the logical product with a checksum-only loop. Worker source tests must freeze this call shape, reset point, and the literal one-call and two-call witness values for the identity path as well as the independently generated values for the non-identity path.
8. Derive the semantic output outside timing from exactly sixteen little-endian `u64` fields: iteration count, checked work, width, workload marker, observed left and right non-identity counts, observed successful callback count, result-derived execution witness, four canonical final-left gate-sequence digest lanes, and four unchanged-right gate-sequence digest lanes. Serialize a gate sequence for hashing as ASCII domain separator `stab.clifford-string.gates.v1`, one zero byte, the width as little-endian `u64`, then exactly one canonical gate-code byte per position. Compute SHA-256 over those bytes and interpret each consecutive eight-byte digest chunk as one little-endian `u64` lane. Require exact Stim/Stab equality and freeze odd and even iteration vectors for both workloads because repeated Clifford composition can cycle back to identity.
9. Validate the final gate-sequence digest with an untimed scalar reference that composes the canonical 24-by-24 table independently of the production string implementation. For the identity workload, verify every position and both non-identity counts remain zero while callback count and execution witness match the exact one-call and two-call vectors. For the non-identity workload, verify the complete first 552-position cross-product, the scale tail, the final left state, right immutability, callback count, and witness. Before worker implementation, add checked source file `benchmarks/fixtures/pq2-clifford-string-vectors.json` with schema version 1, the exact marker values, ordered name and code table, all eight accepted raw 64-byte descriptors and SHA-256 digests, the four tail vectors, and all 36 complete requests in the item-10 order. Each accepted request record must bind its measurement id, iteration count, raw descriptor bytes and digest, and exact sixteen-field output; each rejected request record must bind the same complete input fields, expected rejection class, and unconsumed-barrier outcome. Produce and cross-check those bytes with two independent generators, one using pinned Stim gate conversion and one using the scalar 24-by-24 reference, then treat any vector change as a fixture-schema migration that restarts evidence.
10. Reject zero width, width above 1,048,576, unknown workload marker, a valid marker belonging to the opposite selected workload, wrong measurement id, mismatch between checked `iterations * width` and declared work, malformed descriptor hex, wrong fixture schema, wrong canonical gate count, wrong right-cycle count, wrong cross-product span, wrong public cap, nonzero reserved input, and checked semantic-work overflow before allocation and before consuming an enabled start barrier. The ordered per-worker vector matrix is: accepted `identity-small-odd`, `identity-small-even`, `identity-medium`, `identity-large`, `identity-maximum`, `nonidentity-small-odd`, `nonidentity-small-even`, `nonidentity-medium`, `nonidentity-large`, and `nonidentity-maximum`; rejected `identity-first-over-cap`, `nonidentity-first-over-cap`, `identity-zero-width`, `nonidentity-zero-width`, `unknown-marker`, `identity-wrong-measurement`, `nonidentity-wrong-measurement`, `identity-bad-fixture-schema`, `nonidentity-bad-fixture-schema`, `identity-bad-gate-count`, `nonidentity-bad-gate-count`, `identity-bad-cycle-count`, `nonidentity-bad-cycle-count`, `identity-bad-cross-product-span`, `nonidentity-bad-cross-product-span`, `identity-bad-cap`, `nonidentity-bad-cap`, `identity-reserved`, `nonidentity-reserved`, `identity-work-overflow`, `nonidentity-work-overflow`, `identity-opposite-workload-marker`, `nonidentity-opposite-workload-marker`, `identity-width-work-mismatch`, `nonidentity-width-work-mismatch`, and `identity-malformed-descriptor-hex`. The accepted small-odd, small-even, medium, large, and maximum requests use respectively `(width, iterations)` pairs `(10_000, 1)`, `(10_000, 2)`, `(100_000, 1)`, `(1_000_000, 1)`, and `(1_048_576, 1)` for each workload. Every identity-prefixed rejection must start from the complete `identity-small-odd` request and mutate only the named field; every nonidentity-prefixed rejection must start from the complete `nonidentity-small-odd` request and mutate only the named field. `unknown-marker` starts from `identity-small-odd` and mutates only the marker. Each opposite-workload-marker request retains its selected workload and measurement while substituting the other workload's complete valid descriptor. Each width-work-mismatch request retains its complete small-odd descriptor and changes only declared work from `10_000` to `10_001`. The malformed-hex request retains the identity small-odd request and changes only the final descriptor nibble to non-hex `g`. Use width 1,048,577 for first-over-cap, width `0` for zero-width, marker `u64::MAX` for unknown-marker, the opposite workload's measurement id for wrong-measurement, fixture schema `2`, gate count `23`, cycle counts `1` and `22`, spans `552` and `551`, cap `1_048_575`, reserved value `1`, and iterations `u64::MAX / 10_000 + 1` for work overflow while retaining width 10,000. Freeze the complete request bytes and expected rejection class for every row so a multiply-invalid request or an earlier guard cannot impersonate branch coverage. The canonical 72-receipt nesting order is all 36 Stab requests in the listed order followed by all 36 Stim requests in the same order. There is no separate output-field-overflow branch because every output field is a direct `u64`, a width-bounded count, a checked semantic-work value, or a digest lane; inventing a second unreachable overflow class is forbidden.
11. Add allocation-counted Stab tests proving zero timed allocation calls and bytes for equal-width identity and non-identity callbacks at all six runtime scales and both accepted maxima. Keep construction allocations and setup or peak RSS separate, make no Stim allocation-count claim without instrumentation, and preserve the inherited M12 memory baseline until PQ6 supplies cross-scale replacement evidence.
12. Put the Stab workload in a dedicated `worker/clifford_string.rs` module and invocation validation in `invocation/clifford_string.rs`. Put the pinned comparator in a separately hashed `benchmarks/stim_adapter/clifford_string_contract.h` that uses Stim's public `operator*=` and canonical gate conversion. Keep every source file below 1,200 lines and add each executable child source to the ordered worker or adapter receipt.
13. Bump the private-worker, adapter, contract-preflight, and report schemas when their source or receipt shapes change. Extend the canonical preflight and worker reproducibility matrices with exactly the 72 ordered Clifford invocation receipts defined above; replay must reject an incorrect count and any omitted, reordered, renamed, modified, or cross-worker-transplanted Clifford receipt. The source-current contract uses private Stab build-receipt schema version 5, adapter receipt schema version 11, contract-preflight schema version 12, and qualification report schema version 31.
14. Add `pq2-clifford-string-identity-adapter-smoke` and `pq2-clifford-string-non-identity-adapter-smoke` with source-owned defaults. Each probe must prove exact work and output agreement, right immutability, odd and even behavior, accepted maximum, first rejection, and pre-barrier hostile-input handling. Probe timing remains diagnostic.
15. Apply independent `1.25x` median and bootstrap-upper-bound thresholds at every scale with no waiver path. Use the source-owned `independent-throughput` timing policy only for `PERFQ-M6-CLIFFORD-STRING`, because the exact pinned identity workload intentionally compares Stim's width-proportional public operation with Stab's semantically equivalent O(1) identity-right public fast path and has no bounded common-iteration timing range. Independently calibrate each implementation to the existing 350-millisecond target and 2-second selected-batch ceiling, freeze both selected iteration counts, execute alternating paired samples at those separate counts, and compute each pair's Stab/Stim ratio from seconds per declared single-qubit product. Before those samples, execute both workers at the smaller selected count and require exact workload, input, output, callback, witness, and final-state equality; require the selected calibration output at that same count to equal the common semantic output, require each worker's later output to repeat its own calibration-selected output digest, and bind both counts, both work totals, both output digests, the common semantic receipt, and the source-owned mode into offline replay. The 350-millisecond through 2-second range applies to each selected calibration batch, not to every later warmup or retained sample: those invocations may jitter outside the range but must remain positive, complete within the fixed timeout, repeat the frozen count and output, and remain subject to the source-owned noise and threshold rules. Keep common iterations for memory evidence and for every other runtime group, including `PERFQ-M6-CLIFFORD-STRING-NON-IDENTITY`; never select this mode automatically from an observed ratio, accept a sub-floor selected calibration batch, raise the 20-second common ceiling, pad Stab, weaken the threshold, or use independently timed setup work. Preserve the first faithful non-identity diagnostic even if it fails badly. Profile that exact public lifecycle before optimization; if scalar per-gate composition is the bottleneck, prefer a private six-bit-plane Clifford composition kernel with scalar reference tests and isolated `std::simd` acceleration over architecture-specific intrinsics or an identity-only shortcut. Any storage redesign must preserve every public behavior and restart affected evidence from a clean revision.
16. Map the exact legacy pair `CliffordString_multiplication_10K` / `stab_clifford_string_multiplication_10K` only to `PERFQ-M6-CLIFFORD-STRING/right-multiply-identity` at `small`. Keep the existing M12 timing threshold active until the identity completion receipt passes and replays, then retire only that timing threshold and mapping in a focused migration commit. Do not map the non-identity group to the legacy identity row, and preserve `benchmarks/m12-primary-memory-baseline.json` for PQ6.
17. From one clean pre-migration revision, run all exact CQ prerequisites, private-worker reproducibility, both adapter probes, full and soak reports for both groups at every scale, immediate report replay and regression, separate full and soak rollups for each group, and one completion receipt plus replay per group. Use the identity receipt to authorize only the focused legacy timing migration. Regenerate the complete two-group evidence chain from one clean post-migration revision before claiming current-inventory timing.
18. Finish with milestone audit and independent GPT-5.6/max full code review over the identity and non-identity claim boundary, complete 24-by-23 fixture coverage, independent product oracle, public lifecycle, optimizer barriers, allocations, hostile bounds, schema receipts, migration, failed-result retention, and documentation. Fix every confirmed issue and publish a dedicated progress report with exact hashes, ratios, confidence bounds, allocation outcomes, artifact paths, review closure, and remaining x86-64 and PQ6 scope.

The eleventh slice deliberately qualifies only equal-width public in-place Clifford-string multiplication. It does not qualify allocating multiplication, unequal-width growth, construction, randomization, concatenation, repetition, display, Tableau operations, or any other Algebra path.

### Tests

- Run every row's CQ correctness dependency before timing.
- Verify canonical circuit, DEM, and result-format output digests against pinned Stim.
- Verify bit-kernel outputs against scalar references across all scale and tail classes.
- Verify the popcount fixture's three exact input byte counts and digests, odd and even multi-iteration wrapping checksum accumulation, odd and even final toggle states, the fixed canonical output-digest vectors, actual construction and one-iteration execution at the accepted maximum, the first below-minimum rejection, the first over-cap rejection, and the unaligned-width rejection.
- Verify the popcount adapter probe rejects caller widths below the minimum, above the maximum, or outside the source-owned alignment and accepts the exact medium scale.
- Verify the dense-XOR fixture's two-vector generation rule, three exact combined input byte counts and digests, odd and even final destination states, unchanged source state, fixed canonical output-digest vectors, allocation-free timed mutation, accepted maximum execution, and below-minimum, over-cap, and unaligned rejection classes.
- Verify the dense-XOR adapter probe accepts the exact medium scale and rejects widths outside the source-owned bounded aligned domain before waiting on the start barrier.
- Verify every `not_zero` pattern's three exact fixtures, logical byte counts, frozen input and output digests, accepted unaligned logical width, accepted maximum, below-minimum and over-cap pre-barrier rejection, and zero timed Stab allocations.
- Verify the three `not_zero` adapter probes map to distinct runtime groups and workload IDs, accept 10,000 logical bits by default, reject only out-of-range widths, and never substitute one pattern for another.
- Verify row XOR and item toggle use distinct runtime groups, workload IDs, measurements, work units, output markers, and replacement targets; a receipt or output from one must never validate the other.
- Verify the row fixture contains exactly 1,000 sorted five-item rows before priming, one callback changes only row zero to the exact eight-item symmetric difference, two callbacks restore every row, and all three scales contain exactly 1, 64, or 4,096 complete 1,997-operation callbacks.
- Verify the item fixture uses exactly `2, 5, 9, 5, 3, 6, 10`, one callback produces `2, 3, 6, 9, 10`, two callbacks restore empty state, and all three scales contain exactly 1, 64, or 4,096 complete seven-operation callbacks.
- Verify both sparse-XOR canonical input byte counts and digests, frozen odd, even, and accepted-maximum output digests, partial-sweep and first-over-cap rejection before the start barrier, zero timed Stab allocations after capacity priming, and exact Stim/Stab adapter identity at every timing scale.
- Verify standard common-batch classification at both boundaries; valid wide-ratio classification with either implementation as the slower side, including the exact 20-second boundary; hard rejection below 250 milliseconds, above 20 seconds, when the common-iteration owner exceeds 2 seconds, when equal iteration decisions claim wide-ratio mode, or when both implementations exceed 2 seconds; source-owned independent-throughput classification only for the exact Clifford identity group; independent calibration replay and selected-batch range checks for both sides; acceptance of positive later samples that jitter outside the calibration range while preserving the fixed timeout, count, output, noise, and threshold contracts; common semantic execution at the smaller selected count; selected-output equality with the common proof whenever their counts match; normalized per-work ratio reconstruction with unequal counts; and offline rejection of changed policy, counts, work, outputs, common semantic receipts, selected calibration receipts, or derived ratios.
- Verify every migrated legacy threshold pair has one unique checked replacement target, that the target is an exact implemented primary contract in the same performance feature, and that its measurement ID exists in the executable runtime group. Keep unmapped pairs visible and active.
- Verify the adapter receipt and fingerprint bind an ordered typed set of every compiled comparator source and reject omitted, extra, duplicate, reordered, renamed, modified, or transplanted source evidence.
- Verify algebra benchmarks mutate state and produce nonidentity semantic digests.
- Verify the Clifford identity and non-identity groups have distinct ids, measurements, fixtures, output markers, thresholds, rollups, completion receipts, and replacement dispositions; an identity report must never satisfy the non-identity contract.
- Verify the non-identity Clifford fixture covers all 24-by-23 left/right products in each complete 552-position cycle, preserves the right operand, matches the independent Tableau-backed multiplication table, and binds exact odd, even, scale-tail, accepted-maximum, and first-rejection outputs.
- Verify folded huge-repeat fixtures remain compact and materialized fixtures hit their declared caps.
- Verify each scale increases declared semantic work monotonically.
- Verify the adapter and Stab worker emit the same exact input byte count, input digest, and workload semantic output digest for every timing scale; prove that canonically equivalent but byte-distinct input is rejected by the input-identity contract where canonicalization applies.
- Verify each Stim receipt records the exact ordered CMake-generated `libstim` compile flags and that the adapter compile arguments preserve them, including `-march=native` or explicit SIMD flags where CMake resolves those flags; tampered, omitted, reordered, or injected flags must invalidate the receipt and build fingerprint.
- Verify every normal qualification run and `just bench::qualification-worker-reproducibility` require the canonical worker preflight. The preflight must make both sealed workers confirm source and build identity through the protocol, execute the shared frozen protocol, fixed odd and even popcount vectors, fixed odd and even dense-XOR vectors, fixed early, all-zero, and late `not_zero` vectors, every accepted maximum, the first unsupported circuit-parse scale, and an 83-item partial gate-table sweep with the start barrier enabled and no input. It must invoke each bit workload's applicable below-minimum, unaligned, and over-cap widths with the start barrier enabled and no input; `not_zero` has no alignment rejection. The sixth slice expands the report from 30 to 42 actual receipts with accepted row fields or rejected process digests, and the preflight digest must include both workers' exact source, build-fingerprint, and binary identities. Offline replay must reject omitted, reordered, altered, refingerprinted, stale, or cross-worker-transplanted receipts even when an attacker recomputes the outer preflight digest. The standalone reproducibility command must additionally require two isolated private builds to produce identical source, build-fingerprint, binary-digest, and preflight identities; a dirty checkout must fail before either private build.
- Verify product PR reports are valid but nonpromotable, clean verified full and soak reports are promotable, and regression rejects nonpromotable product reports.
- Verify source-report offline replay rejects checked-inventory drift, runtime-group drift, stale profiler-note content, wrong ownership, and altered input or memory receipts.
- Generate separate full-tier and soak-tier architecture-scoped scale-family rollups that list every required scale and fail closed when a scale report is missing, stale, duplicated, nonpromotable, bound to another commit or inventory digest, produced by different worker source, build, or binary identities, or produced on another architecture.
- Verify formal run and rollup publication reject an existing output directory, a dirty, changed, or source-mismatched producer revision, non-direct or injection-capable artifact names, and source replacement during publication.
- Verify rollup offline replay rejects noncanonical or oversized artifacts, output-path drift, modified source paths or digests, modified timing or memory outcomes, modified aggregate counts, modified producer identity, stale preflight bytes, and compare-and-swap replacement; verify it reconstructs valid failed and noisy families without converting them into passes.
- Verify family outcome precedence is failed when any measurement failed, otherwise noisy when any measurement is noisy, and passed only when every scale measurement passed.
- Verify completion publication rejects an existing output directory, dirty or changed repositories, missing or duplicate scales, colliding or nondirect paths, mixed report workers, stale probe identity, nonpromotable reports, report-only or incomplete regression results, failed or mixed-identity rollups, non-idempotent report or rollup replay, altered artifact byte identities, nonzero step status, malformed step order, stale preflight, noncanonical JSON, and byte-identical source-directory or replay-target inode replacement during publication. Verify the real completion producer makes its new-directory publication durable; verify separately that replay compare-and-swap replacement is durable before bounded cleanup of the old derived tree is treated as best effort.
- Verify completion replay reruns worker reproducibility, the exact source-owned adapter probe, every report replay and regression, and both rollup replays before requiring byte-identical canonical receipt and preflight reconstruction. Do not encode milestone audit or independent code review as a machine-certified step.

### Acceptance Criteria

- Every selected deterministic foundation feature is measured or has a validated non-performance disposition.
- Every comparable row has exact named measurement pairs and three scales where applicable.
- Streaming and folded rows satisfy their declared memory-growth classes.
- Every executable PQ2 product slice completed after completion receipt schema version 1 was introduced has a replayed canonical completion receipt that binds its exact full and soak source reports, regressions, rollups, worker reproducibility, and adapter probe from one clean unchanged revision and one exact CPU identity. Historical slices accepted under the preceding contract remain historical and are not counted as receipt-backed.
- Every ratio above 1.25 has a profiler note and owner and remains a failed target.
- Every measured scale binds exact input identity as well as semantic work and output identity.
- The private Stim and Stab worker builds are byte-reproducible under their source-owned receipts, and each scale-family rollup binds one exact worker identity.
- PQ2 completion requires clean-producer full and soak family rollups plus successful offline rollup replay per native architecture. Linux AArch64 and Linux x86-64 conclusions remain separate until PQ7 reports both; evidence from one architecture cannot close the other.

## PQ3: Qualify Generation And Public CLI Formatting Paths

### Objective

Measure equivalent public process behavior for generation, conversion, serialization, and startup.

### Tasks

1. Qualify `PERF-GENERATION`, `PERF-CONVERT-CLI`, and `PERF-CLI-STARTUP-AND-ERRORS`.
2. Add process-versus-process rows for all implemented generator families and representative convert matrices.
3. Keep construction, encoding, startup, and end-to-end phases separate.
4. Requalify all existing M7 convert rows and replace CLI-body parity labels with process evidence.
5. Add adapter-level `ptb64` comparisons when public Stim command behavior differs but internal format work is equivalent.
6. Add bounded hostile-input latency and memory checks as Stab regressions without pretending malformed-input wall time is a product parity ratio.

### Tests

- Verify exact generator and conversion output before every timing family.
- Verify process arguments, environment, input bytes, output sink, and output byte counts match.
- Verify all generator task and parameter combinations selected by the correctness inventory have a performance disposition.
- Verify startup rows use tiny bounded inputs and throughput rows exceed the startup calibration floor.
- Verify side-output files are consumed and their bytes are included in the output contract.

### Acceptance Criteria

- Public CLI ratio claims are process symmetric.
- Every implemented generator family and every representative result-format mode has at least one full-tier comparison.
- Stab-only public extensions are explicitly split into internal comparable and public report-only evidence where possible.
- Existing M7 benchmark metadata no longer overstates in-process comparisons.

## PQ4: Qualify Sampling, Detection, Conversion, And DEM Sampling

### Objective

Cover the execution engines and streaming command paths across setup, reusable execution, output conversion, and end-to-end phases.

### Tasks

1. Qualify `PERF-SAMPLING`, `PERF-DETECTION`, and `PERF-DEM-SAMPLING`.
2. Build deterministic workload families for each supported semantic gate family, noise shape, repeated circuit, output format, and side-output mode without creating a Cartesian explosion.
3. Split compilation, reference sampling, one-shot latency, batch throughput, conversion, and encoding.
4. Add process CLI rows for `sample`, `detect`, `m2d`, and `sample_dem` over representative small-latency and large-throughput fixtures.
5. Add direct detector-frame, direct conversion, sweep-record, feedback-inlining, replayed-error, and sampled-error submeasurements where those are selected contracts.
6. Add memory slopes for high-shot streaming and high-width reusable buffers.

### Tests

- Require statistical CQ cases to pass before probabilistic timings run.
- Verify deterministic output shape and semantic digest where a seeded stream is not cross-language identical.
- Verify exact shot, measurement, detector, observable, and sampled-error work counts.
- Verify compilation is not repeated inside reusable execution timing.
- Verify output conversion and side-output writing cannot be optimized away.
- Verify large-shot process rows remain bounded by record width rather than total shots.

### Acceptance Criteria

- Every implemented sampling and detection command has setup, steady-state, and end-to-end evidence.
- Every supported result-format family has representative execution-path coverage without requiring every redundant format pair.
- Streaming memory growth matches the declared buffer model.
- Slow comparable rows remain explicit and have owners and profiler notes.

## PQ5: Qualify Analysis, Search, Flows, Utilities, And Transforms

### Objective

Cover variable-complexity algorithms whose work cannot be represented honestly by input bytes alone.

### Tasks

1. Qualify `PERF-ERROR-ANALYSIS`, `PERF-SEARCH-AND-MATCHING`, and `PERF-FLOWS-AND-DETECTOR-UTILITIES`.
2. Add source-owned work counters for analyzed instructions, emitted errors, decomposition attempts, search nodes, explored states, generated flows, solved variables, and reverse-tracker operations where practical.
3. Build easy, representative, adversarial, unsatisfiable, early-exit, bounded-repeat, generated-code, sparse-high-index, and batched workload families.
4. Preserve and expand exact `m10-error-decomp` submeasurement thresholds without aggregating unlike decomposition modes.
5. Add process `analyze_errors` rows and core phase rows for analyzer configuration variants.
6. Add allocation and scaling evidence for sparse trackers, folded DEM traversal, flow generation, matching, and bounded searches.

### Tests

- Verify equal optimum weight, graph or hypergraph digest, DEM semantics, flow result, and search disposition before timing.
- Verify timeout and search-budget exhaustion are distinct from successful early exit.
- Verify work counters are positive, deterministic for fixed fixtures, and comparable across implementations.
- Verify adversarial fixtures actually reach the intended branch through source-owned counters.
- Verify no search row can look faster by returning a weaker result or exploring a smaller unapproved state space.

### Acceptance Criteria

- Every selected analyzer, search, flow, detector-utility, and transform surface has faithful phase evidence or a validated disposition.
- Every search ratio includes both wall time and semantic work counters.
- Every mixed analyzer row uses explicit submeasurement pairs.
- Resource caps and scaling behavior are tested at their documented boundaries.

## PQ6: Graduate Memory, Scaling, And Threshold Evidence

### Objective

Turn qualified workloads into durable regression controls without freezing noisy or misleading thresholds.

### Tasks

1. Run full timing and memory qualification on the designated Linux x86-64 and Linux AArch64 benchmark hosts from a clean committed revision.
2. Classify each measured group as 1.25x pass, 1.25x fail, noisy, report-only, or no-faithful-comparator.
3. Add primary 1.25x thresholds only for faithful rows whose median and upper confidence bound pass and whose evidence is stable across two independent full runs.
4. Add exact submeasurement thresholds for mixed rows and reject row-level thresholds that hide unlike phases.
5. Establish versioned process-memory and Stab allocation baselines from the same selected workload inventory.
6. Add scaling bounds for streaming, compact traversal, materialization, and search work.
7. Keep existing M12 thresholds until replacement rows have equal or stronger coverage and a checked migration record. Every retirement must bind the exact legacy threshold and replacement target, clean authorization revision and inventory, completion report and preflight digests, migration revision and inventory, and any retained memory baseline in `benchmarks/qualification-threshold-migrations.json`; inventory validation must fail when a retired row is missing authorization or a ledger entry refers to a reopened row.
8. Select an explicit public resource contract for programmatically constructed circuits deeper than the parser's 256-level repeat-nesting limit: either iterative serialization and destruction with bounded work or a fallible depth-checked construction or serialization boundary.

### Tests

- Reject threshold entries without two qualifying clean reports, matching fingerprints, exact measurement ids, and current fixture digests.
- Reject waivers for comparable or slow rows and fail unused waivers.
- Reject memory baselines from timing builds or timing thresholds from memory builds.
- Reject scale families with missing sizes, nonmonotonic work, changed semantics, or insufficient range.
- Verify threshold migration preserves every old primary feature or records a reviewed supersession, rejects refingerprinted or stale authorization receipts, rejects a missing or mismatched replacement scale, rejects a reopened legacy timing row, and retains any independently owned memory baseline.
- Test maximum-accepted and first-rejected programmatic repeat depth for string and file serialization, early writer failure, and destruction without process stack exhaustion.

### Acceptance Criteria

- Every comparable measured group has an explicit target result and no slow row is waived.
- Every primary threshold is backed by two clean full reports.
- Every streaming or compact group has a machine-checked growth result.
- Programmatic circuit serialization has an explicit tested repeat-depth resource contract independent of parser admission.
- Current M12 coverage is preserved or superseded by stronger named evidence.

## PQ7: Final Qualification, Audit, And Reporting

### Objective

Prove that the suite is complete, honest, reproducible, and synchronized with the feature and correctness inventories.

### Tasks

1. Run schema, full, and soak tiers from clean committed revisions on the designated Linux x86-64 and Linux AArch64 host profiles.
2. Generate a source-owned `docs/plans/comprehensive-qualification-report.md` summarizing coverage, build fingerprints, pass and failure counts, no-ratio dispositions, memory results, scaling results, and exact artifact paths.
3. Update `docs/stab-feature-checklist.md`, `docs/plans/stim-test-porting-plan.md`, `benchmarks/README.md`, the roadmap, and benchmark metadata to reflect implemented evidence without overstating parity.
4. Run `milestone-audit` over CQ0 through CQ6 and PQ0 through PQ6 and fix every implementation or evidence issue.
5. Log only genuinely revealed under-specification in `docs/plans/milestone-spec-gaps.md` and resolve it before final qualification when it affects a selected contract.
6. Run `full-code-review` over correctness, benchmark, adapter, hostile-input, statistical, performance, operational, and documentation changes and fix every confirmed finding.

### Tests

- Re-run every final report from recorded commands and verify source and fixture digests.
- Validate that all benchmark groups reference passing correctness cases.
- Validate that feature, correctness, and performance inventories are bijective over the selected scope.
- Validate all source-owned reports use clean revisions and the pinned Stim commit.
- Run the final standard checks listed below.

### Acceptance Criteria

- The performance disposition ledger has no missing, duplicate, stale, or ambiguous feature.
- Every measured group has faithful evidence or an explicit failed parity result.
- Every measured group has separate full-tier x86-64 and AArch64 evidence or an exact platform-specific disposition that does not erase support on the other architecture.
- Every no-ratio disposition is machine-checked and names the condition that would retire it.
- The report states suite completeness separately from timing parity, memory status, and optimization backlog.
- Audit and review have no unresolved confirmed finding.

## Required Verification

Run focused tests during each milestone, then run the following before final qualification:

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings
cargo test --workspace --quiet
just qualification::correctness-check
just qualification::correctness-run --tier full
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

The qualification commands in this section are planned commands and become mandatory as their owning milestones implement them.

## Defect And Under-Specification Policy

- Fix correctness, work-equivalence, runner, timeout, memory, statistical, and documentation defects before accepting benchmark evidence.
- Treat a ratio change caused by less work, skipped output, weaker semantics, or changed caps as a correctness failure, not an optimization.
- Do not relax the 1.25x target, confidence rule, scale, sample count, or fixture after seeing an unfavorable result.
- A newly discovered feature or materially distinct workload must receive a stable inventory id and acceptance contract before implementation continues.
- Record genuine under-specification in `docs/plans/milestone-spec-gaps.md`; do not use that file to postpone a decision already required by this plan.

## Final Deliverable

The final comprehensive qualification report must include:

- Feature disposition totals and measured workload totals by domain.
- Existing rows retained, reworked, superseded, and removed.
- Comparable measurements passing and failing the 1.25x target.
- Median speedup or slowdown distributions without averaging heterogeneous work.
- The fastest and slowest named measurement pairs with confidence intervals.
- No-faithful-comparator rows and their retirement conditions.
- Process memory ratios, Stab allocation regressions, and scaling classifications.
- Every failed or noisy row with profiler evidence, owner, and next action.
- Exact commands and artifact paths needed to reproduce the source-owned evidence.
