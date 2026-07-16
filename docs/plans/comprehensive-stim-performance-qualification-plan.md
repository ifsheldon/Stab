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

Generated artifacts belong below `target/benchmarks/qualification/` and must never be treated as source-owned baselines merely because a local run succeeded.

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

### Timing And Statistics

- Calibrate each timed batch to at least 250 milliseconds and at most 2 seconds without exceeding the workload's declared iteration or memory cap.
- Controllers may target a higher source-owned calibration duration to absorb ordinary run-to-run jitter, but must record that target separately and independently reject the retained common batch when it falls below 250 milliseconds or exceeds 2 seconds. PQ1 uses a 350-millisecond target and retains the 250-millisecond acceptance floor.
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

Evidence: [pq0-performance-disposition-progress-report.md](pq0-performance-disposition-progress-report.md), [pq2-circuit-parse-qualification-progress-report.md](pq2-circuit-parse-qualification-progress-report.md), [pq2-circuit-canonical-print-qualification-progress-report.md](pq2-circuit-canonical-print-qualification-progress-report.md), [pq2-gate-name-hash-qualification-progress-report.md](pq2-gate-name-hash-qualification-progress-report.md), [pq2-simd-word-popcount-qualification-progress-report.md](pq2-simd-word-popcount-qualification-progress-report.md), [pq2-simd-bits-xor-qualification-progress-report.md](pq2-simd-bits-xor-qualification-progress-report.md), and `benchmarks/stim-qualification-suite.json` at current performance digest `8b4735f7d651e74d3029014a4bf0c4580d85462295d66c93d8e090d3433c3958`, bound to correctness digest `ccb80eb4b660a375b59460c3b7fa03a810abd6f868735b566735378105db22b2`. `PERFQ-M4-CIRCUIT-PARSE`, `PERFQ-M4-CIRCUIT-CANONICAL-PRINT`, `PERFQ-M4-GATE-LOOKUP`, `PERFQ-M5-SIMD-WORD`, and `PERFQ-M5-SIMD-BITS` are implemented exact runtime contracts with exact `1.25x` targets. Their passing AArch64 reports remain historical after later inventory changes. Dense-XOR evidence from clean revision `5d226c94ece70f96d0b771f9c8cde7464ccd261b` passed all full and soak scales at its original inventory, with median ratios from `0.374633x` to `0.559828x` and worst confidence-interval upper bound `0.561257x`. The legacy canonical-print diagnostic is superseded and no longer owns beta, timing-regression, or memory waivers. The legacy XOR threshold pair now has an exact executable replacement mapping; the `m5-simd-word` and `not_zero` thresholds remain active until their own migration evidence lands.

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

Audit note: the parent must independently derive `iterations * work_items`, keep calibration probes work-bound and outside ratio evidence, perform semantic preflight at the exact common calibrated batch shape, bind every subsequent validation, warmup, sample, and memory receipt to that digest, and inspect the clean revision through a config-free private Git view tied to an exact captured commit. Offline validation must replay the calibration algorithm from raw measured and process-wall durations, bind wrapper and row iterations, enforce the exact workload and measurement identities for every phase, and reproduce repeated memory fields from raw invocation receipts. Both qualification workers must be rebuilt from materialized committed source in fresh private targets, bind canonical tool, argument, environment, input, fingerprint, and binary identities into reconstructable receipts, and execute from sealed copies. Controlled host evidence requires an exclusive full-run profile-and-CPU lease, stable thermal-zone identity and readings no higher than the profile limit whenever the platform exposes the required probes, and offline replay of the source-owned policy instead of trusting serialized `verified` or violation fields.

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

- Unit-test calibration lower and upper bounds, zero-duration handling, overflow, timeouts, and maximum iterations.
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

Status: Active as of 2026-07-16. All 271 selected CQ2 parents have complete exact ownership. Five exact runtime groups are implemented at performance inventory `8b4735f7d651e74d3029014a4bf0c4580d85462295d66c93d8e090d3433c3958`. Parser, canonical printing, gate-name hashing, SIMD-word popcount, and complete-vector XOR have historical passing controlled-host evidence under preceding inventories. The remaining model, format, kernel, and algebra groups are still planned.

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
11. Publish each scale-family rollup only from a clean unchanged checkout whose commit exactly equals the source reports' Stab commit, record that producer state separately from the source-report identity, require exact Stim and Stab worker source, build-fingerprint, and binary-digest identity across every scale, and bound each source report and preflight while retaining only the reduced rollup evidence needed for the family.
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
7. Use fixed one-iteration and two-iteration 4,096-bit vectors to prove odd and even final states and fixed canonical output digests. Execute one iteration at the accepted maximum of 268,435,456 bits. Reject widths below 256 bits, widths not divisible by 256 bits, and widths above 268,435,456 bits before allocation and before reading the start barrier. Contract-preflight schema version 4 must record all 30 actual accepted and rejected receipts for both sealed workers, qualification report schema version 19 must bind that expanded preflight and adapter receipt, and standalone reproducibility must repeat the complete preflight for two isolated builds.
8. Record setup and peak process RSS separately at every scale as report-only observations. Two preallocated vectors must remain live during timing, the source must remain unchanged, and allocator instrumentation must prove that the Stab timed mutation allocates zero calls and zero bytes at the small, medium, large, and accepted-maximum widths. Pinned Stim source inspection proves only the isolated in-place comparator shape and does not establish a Stim allocation count. This slice makes no cross-scale RSS, cross-implementation allocation, or Stim-relative memory claim; PQ6 owns the explicit growth rule.
9. Apply the unchanged `1.25x` median and bootstrap upper-confidence-bound threshold to `xor-complete-vector` at every scale with no waiver path. A failed or noisy clean result must remain visible with a source-owned profiler note and next owner action.
10. Reclassify the inherited heterogeneous `m5-simd-bits` row from retained to reworked only after the exact XOR replacement enters the checked inventory. Keep the existing M12 XOR and `not_zero` submeasurement thresholds active until later migration evidence explicitly replaces each one, and do not imply that this XOR slice qualifies `not_zero` or the unmatched Stab-only logical operations.
11. Split production helpers or tests before `worker.rs`, `invocation.rs`, or `adapter.rs` crosses 1,200 lines. Any helper moved out of the sealed worker or adapter source must be included in the corresponding source identity and build fingerprint instead of becoming unbound executable behavior.
12. From one clean committed revision, run both exact CQ prerequisites, private-worker reproducibility, the dense-XOR adapter probe, full and soak reports at every scale, immediate offline replay and regression checks, and separate AArch64 scale-family rollups. Run milestone audit and GPT-5.6/max full code review before recording completion evidence.

The fifth slice owns one dense XOR phase only. `simd_bits_not_zero_100K`, randomization, masked and ranged mutation, copy, clear, bit-table operations, sparse XOR, and all other remaining bit-kernel groups stay planned or retain their exact historical diagnostic evidence until separately graduated.

The fifth runtime contract is complete on controlled Linux AArch64 at performance inventory `fb50789c58786219c807c79e87202396b17563ee7cb584bcda4d3379007ed716` and correctness inventory `ccb80eb4b660a375b59460c3b7fa03a810abd6f868735b566735378105db22b2`. Clean revision `5d226c94ece70f96d0b771f9c8cde7464ccd261b` binds both exact CQ prerequisites, the ordered typed comparator-source receipt, 30 actual contract probes, reproducible sealed workers, six passing non-noisy source reports, six successful report replays and regression checks, and separate replayed full and soak AArch64 rollups. Median ratios range from `0.374633x` to `0.559828x`, with worst confidence-interval upper bound `0.561257x`. Milestone audit found no implementation, comparator-fidelity, schema, resource-boundary, M12-threshold, or performance-gate defect; it found and closed command-record and documentation-synchronization defects and logged two genuine under-specifications. The independent GPT-5.6/max full code review found and closed stale current-evidence wording, an incorrect digest label, and an incorrect CQ-owner attribution, with no confirmed code or evidence-integrity defect. Native Linux x86-64 execution and all remaining PQ2 groups remain unclaimed.

The source-current performance inventory is `8b4735f7d651e74d3029014a4bf0c4580d85462295d66c93d8e090d3433c3958`. It adds an exact source-owned replacement contract from the legacy `simd_bits_xor_10K` and `stab_simd_bits_xor_10K` threshold pair to executable measurement `PERFQ-M5-SIMD-BITS/xor-complete-vector`. Validation rejects stale or duplicate source pairs, duplicate targets, cross-feature or nonpromotable targets, missing primary mappings, and runtime measurement IDs absent from the executable group contract. The unmapped `not_zero` threshold remains active. This inventory-only hardening makes all earlier timing reports historical without changing their outcomes. Qualification report schema version 20 embeds private Stab build-receipt schema version 2, whose ordered framed source collection binds both `worker.rs` and its extracted bit-kernel module. Stab zero-allocation evidence now covers every dense-XOR runtime scale and the accepted maximum; no Stim allocation claim is made. Before the next slice adds workload behavior, finish the completion-receipt contract.

### Tests

- Run every row's CQ correctness dependency before timing.
- Verify canonical circuit, DEM, and result-format output digests against pinned Stim.
- Verify bit-kernel outputs against scalar references across all scale and tail classes.
- Verify the popcount fixture's three exact input byte counts and digests, odd and even multi-iteration wrapping checksum accumulation, odd and even final toggle states, the fixed canonical output-digest vectors, actual construction and one-iteration execution at the accepted maximum, the first below-minimum rejection, the first over-cap rejection, and the unaligned-width rejection.
- Verify the popcount adapter probe rejects caller widths below the minimum, above the maximum, or outside the source-owned alignment and accepts the exact medium scale.
- Verify the dense-XOR fixture's two-vector generation rule, three exact combined input byte counts and digests, odd and even final destination states, unchanged source state, fixed canonical output-digest vectors, allocation-free timed mutation, accepted maximum execution, and below-minimum, over-cap, and unaligned rejection classes.
- Verify the dense-XOR adapter probe accepts the exact medium scale and rejects widths outside the source-owned bounded aligned domain before waiting on the start barrier.
- Verify every migrated legacy threshold pair has one unique checked replacement target, that the target is an exact implemented primary contract in the same performance feature, and that its measurement ID exists in the executable runtime group. Keep unmapped pairs visible and active.
- Verify the adapter receipt and fingerprint bind an ordered typed set of every compiled comparator source and reject omitted, extra, duplicate, reordered, renamed, modified, or transplanted source evidence.
- Verify algebra benchmarks mutate state and produce nonidentity semantic digests.
- Verify folded huge-repeat fixtures remain compact and materialized fixtures hit their declared caps.
- Verify each scale increases declared semantic work monotonically.
- Verify the adapter and Stab worker emit the same exact input byte count, input digest, and workload semantic output digest for every timing scale; prove that canonically equivalent but byte-distinct input is rejected by the input-identity contract where canonicalization applies.
- Verify each Stim receipt records the exact ordered CMake-generated `libstim` compile flags and that the adapter compile arguments preserve them, including `-march=native` or explicit SIMD flags where CMake resolves those flags; tampered, omitted, reordered, or injected flags must invalidate the receipt and build fingerprint.
- Verify every normal qualification run and `just bench::qualification-worker-reproducibility` require the canonical worker preflight. The preflight must make both sealed workers confirm source and build identity through the protocol, execute the shared frozen protocol, fixed odd and even popcount vectors, fixed odd and even dense-XOR vectors, both accepted maxima, the first unsupported circuit-parse scale, and an 83-item partial gate-table sweep with the start barrier enabled and no input, and invoke each bit workload's below-minimum, unaligned, and over-cap widths with the start barrier enabled and no input. The fifth slice expands the report from 18 to 30 actual receipts with accepted row fields or rejected process digests, and the preflight digest must include both workers' exact source, build-fingerprint, and binary identities. Offline replay must reject omitted, reordered, altered, refingerprinted, stale, or cross-worker-transplanted receipts even when an attacker recomputes the outer preflight digest. The standalone reproducibility command must additionally require two isolated private builds to produce identical source, build-fingerprint, binary-digest, and preflight identities; a dirty checkout must fail before either private build.
- Verify product PR reports are valid but nonpromotable, clean verified full and soak reports are promotable, and regression rejects nonpromotable product reports.
- Verify source-report offline replay rejects checked-inventory drift, runtime-group drift, stale profiler-note content, wrong ownership, and altered input or memory receipts.
- Generate separate full-tier and soak-tier architecture-scoped scale-family rollups that list every required scale and fail closed when a scale report is missing, stale, duplicated, nonpromotable, bound to another commit or inventory digest, produced by different worker source, build, or binary identities, or produced on another architecture.
- Verify rollup publication rejects a dirty, changed, or source-mismatched producer revision, non-direct or injection-capable artifact names, and source replacement during publication.
- Verify rollup offline replay rejects noncanonical or oversized artifacts, output-path drift, modified source paths or digests, modified timing or memory outcomes, modified aggregate counts, modified producer identity, stale preflight bytes, and compare-and-swap replacement; verify it reconstructs valid failed and noisy families without converting them into passes.
- Verify family outcome precedence is failed when any measurement failed, otherwise noisy when any measurement is noisy, and passed only when every scale measurement passed.

### Acceptance Criteria

- Every selected deterministic foundation feature is measured or has a validated non-performance disposition.
- Every comparable row has exact named measurement pairs and three scales where applicable.
- Streaming and folded rows satisfy their declared memory-growth classes.
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
7. Keep existing M12 thresholds until replacement rows have equal or stronger coverage and an explicit migration record.
8. Select an explicit public resource contract for programmatically constructed circuits deeper than the parser's 256-level repeat-nesting limit: either iterative serialization and destruction with bounded work or a fallible depth-checked construction or serialization boundary.

### Tests

- Reject threshold entries without two qualifying clean reports, matching fingerprints, exact measurement ids, and current fixture digests.
- Reject waivers for comparable or slow rows and fail unused waivers.
- Reject memory baselines from timing builds or timing thresholds from memory builds.
- Reject scale families with missing sizes, nonmonotonic work, changed semantics, or insufficient range.
- Verify threshold migration preserves every old primary feature or records a reviewed supersession.
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
