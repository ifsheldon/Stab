# Milestone Under-Specification Log

This log records milestone loopholes, ambiguous acceptance criteria, and under-specified scope discovered during milestone implementation or milestone audit.
Use this file for specification gaps only.
Implementation defects, missing tests, benchmark failures, documentation omissions, and code-review findings should be fixed in the milestone work unless a separate follow-up is explicitly accepted.

## Entry Format

```text
## YYYY-MM-DD - Mx: Milestone Title

Status: Open | Resolved | Superseded
Revealed by: implementation, test, benchmark, audit, or review evidence
Current text: the milestone wording that was too weak or ambiguous
Gap: what the milestone failed to specify
Proposed amendment: concrete replacement text or additional done criterion
Resolution: link or note for the plan update that resolved the gap
```

## Open Entries

## 2026-07-04 - RPF2: Flow-Time-Reversal Dependency Boundary

Status: Open
Revealed by: implementation of the RPF2 circuit transform slices.
Current text: RPF2 asks to implement `time_reversed_for_flows` only after RPF5 defines required flow semantics, while also listing flow-time-reversal under the RPF2 transform objective.
Gap: RPF2 now has a scoped unitary `time_reversed_for_flows` bridge, but it still cannot specify or test the measurement-rich form without the RPF5 measurement-rich `Flow`, `has_flow`, flow-generator, included-observable, and measurement-index semantics. Treating the missing measurement-rich rewrite as an RPF2 implementation defect would force agents to invent flow semantics before the owning milestone defines them.
Proposed amendment: keep measurement-rich `time_reversed_for_flows` manifest-only under RPF2 until RPF5 closes the measurement-rich flow contract, then add a follow-up transform slice with exact public API shape, flow-semantic tests, and benchmark classification.
Resolution: Pending RPF5 measurement-rich flow semantics.

## 2026-07-04 - PF1: Path-Based Circuit File Helper Streaming Boundary

Status: Open
Revealed by: full-code-review of the PF1 circuit file-helper API slice.
Current text: PF1 asks for circuit file constructor and writer helpers where they are useful Rust APIs, but it does not define whether path-based Rust helpers must stream through the parser or may use a bounded string-backed parser until a streaming `.stim` parser exists.
Gap: `Circuit::write_stim_file` can stream canonical output through an `io::Write`, but `Circuit::from_stim_file` still delegates to the existing string-backed parser. The current Rust API rejects files larger than 64 MiB before parsing to avoid unbounded allocation, so it is bounded but not a full replacement for Stim v1.16.0's streaming `FILE*` reader.
Proposed amendment: keep path-based Rust file helpers in PF1 with the documented 64 MiB read cap, and add a later parser milestone before claiming unbounded streaming `.stim` file-read parity for Rust APIs or future bindings.
Resolution: Pending future streaming parser milestone.

## 2026-07-04 - PF1: Rust Coordinate Query Non-Finite Results

Status: Open
Revealed by: full-code-review of the PF1 circuit detector-coordinate API slice.
Current text: PF1 requires Rust circuit coordinate query parity for final qubit coordinates and detector coordinates, but it does not define whether Rust APIs should exactly mirror Stim v1.16.0 C++ double-overflow behavior or reject non-finite folded coordinate results.
Gap: Stim v1.16.0's C++ coordinate helpers can return infinities when finite coordinate inputs overflow during folded repeat arithmetic, while Stab's current Rust coordinate APIs reject non-finite folded coordinate results as a deliberate hardening choice.
Proposed amendment: keep the Rust API hardening documented for PF1, and require a later binding-parity decision before claiming exact Python or C++ coordinate-query side-effect parity.
Resolution: Pending future binding-parity decision.

## 2026-07-04 - M9: Exact Feedback Loop Refolding Boundary

Status: Open
Revealed by: implementation of `docs/plans/m9-m2d-sweep-feedback-parity-plan.md`.
Current text: the M9 sweep and feedback plan asks to implement `--ran_without_feedback` and port transform subcases for `basic`, `demolition_feedback`, `loop`, `mpp`, and interleaved feedback ordering, while allowing unfinished transform subcases to be logged precisely.
Gap: the implemented M9 slice supports the public command-level `m2d --ran_without_feedback` case, exact `basic`, exact `demolition_feedback`, exact MPP feedback-transform parity, interleaved-operation ordering, and sweep-control preservation, but it does not claim full `Circuit.with_inlined_feedback` parity. Exact loop refolding is not implemented.
Proposed amendment: define this M9 wave as command-level feedback-inlining parity plus the exact transform subcases now covered by source-owned tests. Add a later transform milestone for exact loop refolding and public transform API parity before the checklist can mark full feedback-inlining transform parity done.
Resolution: Pending future transform milestone.

## Resolved Entries

## 2026-06-28 - M12: CLI Sampling And Input Resource Boundaries

Status: Resolved
Revealed by: final GOAL full-code-review of public CLI resource handling.
Current text: M12 required allocation tracking, sampler hot-path optimization, memory gates, and future streaming detection conversion, but did not explicitly require the public `sample` CLI to avoid materializing all generated shots or require every implemented public CLI input path to use a bounded reader or streaming parser.
Gap: `stab sample` could build the full output buffer before writing, and some public `sample`, `convert`, `detect`, and `m2d` circuit or result-input reads bypassed the existing bounded input helper. The benchmark and memory-gate criteria did not by themselves prove hostile-input or huge-output CLI behavior.
Proposed amendment: add a M12 task and done criterion requiring public CLI resource-boundary regression tests: generated sample output must stream through the writer in bounded chunks, and implemented public circuit or result-input reads must have documented caps or streaming readers.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now requires CLI resource-boundary hardening in M12. The implementation adds `CompiledSampler::for_each_sample_with_seed_and_reference_mode`, streams `stab sample` output per shot or per 64-shot `ptb64` group, and routes public `sample`, `convert`, `detect`, and `m2d` circuit or result-input reads through documented 64 MiB caps unless the command already has a narrower streaming or bounded reader. Evidence includes `cargo test -p stab-core sampling streaming_samples_match_seeded_record_samples`, `cargo test -p stab-cli sample_streams_output_without_materializing_all_shots`, and `cargo test -p stab-cli oversized`.

## 2026-06-28 - M12: Optimization Log Evidence Strength

Status: Resolved
Revealed by: fresh M12 milestone audit of the source-owned optimization log.
Current text: M12 says rows optimized below the final-current profiler-note threshold must be listed in `benchmarks/profiler-notes/m12/optimization-log.json` with before and after report paths, dominant-cost evidence or a profiler blocker, implementation summary, semantic checks, and follow-up policy.
Gap: the validator checked schema shape, safe ids, safe report paths, non-empty evidence fields, and required row coverage, but it did not prove that the referenced before and after reports contain the row or support the claimed threshold or gate status.
Proposed amendment: either extend the source-owned optimization log with machine-checkable before and after ratios/statuses that do not depend on local `target/` artifacts, or add an ops validator mode that checks referenced reports when the reports are archived alongside completion evidence.
Resolution: `benchmarks/profiler-notes/m12/optimization-log.json` now uses schema version 2 with source-owned before and after ratios, gate statuses, hot-path statuses, and source profiler-note paths for after rows still above 1.5x. `cargo test -p stab-bench m12_optimization_log_validates_source_file` validates the new schema and required row coverage, while `docs/plans/rust-stim-drop-in-rewrite.md` and `benchmarks/README.md` now describe the stronger optimization-log evidence contract.

## 2026-06-28 - M10: ErrorMatcher Repeat-Contained Noise Scope

Status: Resolved
Revealed by: milestone audit and full-code-review of the M10 detector-analysis implementation.
Current text: M10 linked `src/stim/simulators/error_matcher.test.cc` as an analyzer test source and required structural oracle rows, but the roadmap did not say whether the milestone had to port every upstream ErrorMatcher provenance case.
Gap: current Stab error matching covers a staged subset, while upstream repeat-contained noise stack frames, generated surface-code repeat matching, heralded matching, and full sparse reverse tracker consumption require broader detector-analysis provenance work than the M10 done criteria named.
Proposed amendment: define M10 ErrorMatcher acceptance as the implemented staged direct-Rust subset and add a future detector-analysis item for the remaining provenance cases before claiming full ErrorMatcher parity.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now states that M10 accepts the implemented `coverage-simulators-error-matcher` subset only, names repeat-contained noise stack frames and related generated-circuit cases as future detector-analysis work, and adds that work to the Future Plan.

## 2026-06-28 - M10: Initial Analyzer Resource Limits

Status: Resolved
Revealed by: full-code-review of M10 public DEM analysis and `stab analyze_errors` input handling.
Current text: M10 required loop folding without accidental high-repeat flattening and linked graphlike, hypergraph, SAT, and analyzer workflows, but it did not specify temporary resource limits for the first compatible implementation.
Gap: without explicit caps, public APIs could try to flatten huge DEM repeats and `stab analyze_errors` could accept oversized circuit input or deeply nested repeats before the milestone had streaming or folded traversal support.
Proposed amendment: document accepted temporary limits for CLI input, circuit parser nesting, and DEM flattening-heavy analysis paths, require rejection tests for those limits, and require future streaming or folded traversal evidence before relaxing them.
Resolution: M10 now documents a 64 MiB `analyze_errors` input cap, a 1,000,000 line circuit parser cap, a 256 repeat-nesting cap, and DEM analysis flattening caps of 100,000 repeats, 1,000,000 expanded instructions, and 1,000,000 expanded repeat iterations. Evidence includes `analyze_errors_rejects_oversized_input_file_before_reading`, `analyze_errors_rejects_excessive_repeat_nesting`, `parser_rejects_excessive_repeat_nesting`, `dem_counts_large_repeat_detectors_without_unrolling`, `dem_public_flattening_apis_reject_excessive_repeat_expansion`, and `sat_problem_rejects_excessive_repeat_expansion`.

## 2026-06-28 - M10: Benchmark Evidence Reproducibility

Status: Resolved
Revealed by: milestone audit and full-code-review of M10 benchmark completion evidence.
Current text: M10 required `just bench::compare --milestone M10` to report `.dem` parse/print and `analyze_errors` workloads with loop-folding cases included.
Gap: a bare non-strict compare can succeed while rows are missing from the selected pinned-Stim baseline, and a progress report can cite a stale local baseline path after benchmark row ownership changes.
Proposed amendment: treat bare M10 benchmark comparison as reportable Stab-side evidence, but require a current selected pinned-Stim baseline path and matching strict compare report whenever completion evidence claims strict Stab-vs-Stim benchmark comparison.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now requires fresh named baseline and strict compare paths for strict M10 benchmark claims. `docs/plans/m10-progress-report.md` cites `target/benchmarks/m10-goal-baseline/baseline.json` and `target/benchmarks/m10-goal-strict-compare`, regenerated after the audit found the stale baseline.

## 2026-06-28 - M12: Resident Memory Peak Wording

Status: Resolved
Revealed by: fresh M12 milestone audit of memory-gate evidence.
Current text: the M12 done criterion said no primary workload may regress peak allocations or resident memory by more than 25 percent, while the implementation records allocation-counter maxima and samples process resident memory around each Stab-side benchmark measurement.
Gap: the wording could be read as requiring true peak RSS tracking during the operation, but the implementation and reports provide sampled resident-memory evidence.
Proposed amendment: describe the memory gate as peak live allocation evidence plus sampled resident-memory evidence, or replace the sampler with true peak-RSS tracking.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now says completion-style memory runs fail rows missing sampled resident-memory evidence and distinguishes historical schema-version-1 absolute sampled resident-memory checks from schema-version-2 row-local resident-delta checks with a 64 KiB absolute slack for page-granular RSS sampling noise. The done criterion now names sampled resident deltas for schema-version-2 memory evidence instead of unqualified resident memory.

## 2026-06-28 - M12: Profile Evidence Timing

Status: Resolved
Revealed by: milestone audit of M12 profiler-note evidence.
Current text: M12 says to profile every benchmark that is slower than the beta gate before optimizing it, and source-owned compare runs require notes for rows slower than 1.5x pinned Stim.
Gap: the milestone does not say whether completion evidence requires pre-optimization profiler captures, final-current profiler notes for rows still slower than 1.5x, or both.
Proposed amendment: choose a durable rule: either require pre-optimization notes for every row optimized during M12, or require final-current notes only for rows still slower than 1.5x and separate optimization logs for rows that were fixed.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now defines final-current profiler-note evidence for rows still slower than 1.5x pinned Stim, plus source-owned optimization-log evidence for M12-optimized rows. `benchmarks/profiler-notes/m12/optimization-log.json` records before and after reports, source-owned ratios, gate statuses, hot-path statuses, dominant-cost evidence, implementation summaries, semantic checks, and follow-up policy for optimized rows, and `cargo test -p stab-bench m12_optimization_log_validates_source_file` validates the log shape and required row coverage.

## 2026-06-28 - M12: Memory Gate Metric Scope

Status: Resolved
Revealed by: milestone audit of M12 memory-gate evidence.
Current text: M12 says no primary workload may regress peak allocations or resident memory by more than 25 percent relative to the first complete Stab benchmark report.
Gap: the previous memory gate tracked Stab-side allocation counts and maximum live allocated bytes, but it did not measure resident set size.
Proposed amendment: either narrow the done criterion to allocation counts and maximum live allocated bytes, or add RSS measurement to the benchmark report and memory gate before M12 completion.
Resolution: `stab-bench compare --track-allocations` now samples Stab-side resident memory with `memory-stats`, records both `resident_bytes` and `resident_delta_bytes` on measurements, promotes `stab_resident_bytes_max` and `stab_resident_delta_bytes_max` to compare rows, and `--require-memory-gate` requires allocation evidence plus the schema-selected resident-memory evidence. The historical M12 completion run passed with all 71 rows in `memory_gate_status=pass`, and the current post-beta `benchmarks/m12-primary-memory-baseline.json` uses schema version 2 with `stab_allocation_bytes_max`, `stab_resident_bytes_max`, and `stab_resident_delta_bytes_max` for the expanded 85-row primary matrix.

## 2026-06-28 - M12: Regression Threshold Automation

Status: Resolved
Revealed by: milestone audit of M12 regression-threshold evidence.
Current text: M12 says workloads already at or below 1.25x Stim have benchmark thresholds checked by CI smoke or scheduled benchmark automation.
Gap: the repository has source-owned threshold files and local `just bench::primary-regression` evidence, but no checked CI or scheduled automation currently runs the full threshold gate.
Proposed amendment: add a CI or scheduled benchmark workflow for the full threshold gate, or revise the done criterion to accept archived local reports plus a lighter CI smoke command.
Resolution: `.github/workflows/m12-benchmarks.yml` now runs weekly and by manual dispatch, records a fresh `just bench::baseline --primary` report, runs `just bench::primary-regression --baseline <fresh-baseline> --report target/benchmarks/m12-scheduled-primary-regression`, and uploads the generated baseline and compare reports. `just bench::primary-regression` now includes `--warmup --measurement-runs 3` so the scheduled threshold gate uses the same warmed median-of-three Stab-side evidence policy as completion-style timing runs.

## 2026-06-28 - M12: Primary Row Comparability Classes

Status: Resolved
Revealed by: milestone audit of M12 beta-gate evidence and full code review of direct-match benchmark rows.
Historical text at the time: M12 said comparable primary rows must pass the 2.0x beta gate, and measured `contract-only` rows may pass only with source-owned waivers.
Gap: the milestone does not define the allowed comparability classes precisely enough for mixed rows such as direct internal perf matches, public CLI baselines, contract-representative in-process measurements, report-only rows, partial matches, and contract proxies.
Proposed amendment: define benchmark comparability classes such as `direct-match`, `cli-baseline`, `contract-representative`, `report-only`, `partial-match`, and `contract-proxy`; state which classes may satisfy beta, which require waivers, and which must remain follow-up evidence only.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` and `benchmarks/README.md` now define the M12 benchmark comparability taxonomy, `stab-bench compare` records `comparability` in compare rows, positional submeasurement pairing is limited to `direct-match`, beta-waiver diagnostics include the class, and `primary_compare_rows_have_machine_readable_comparability_classes` rejects unclassified primary rows.

## 2026-06-28 - M12: Microbenchmark Warmup And Repeated-Run Evidence

Status: Resolved
Revealed by: repeated M12 primary beta runs after adding the M4 sparse parser fast path.
Current text: M12 requires completion-style performance runs to pass the source-owned beta gate, but it does not define warmup, retry, repeated-run, or median-of-runs evidence for sub-microsecond and first-row benchmark cases.
Gap: the focused M4 parser row repeatedly measured around 1.31x pinned Stim and the next full primary beta run passed, but one intervening full primary beta run transiently measured `m4-circuit-parse` above the historical 2.0x gate at the beginning of the report.
Proposed amendment: define a completion evidence policy for tiny benchmark rows, such as one warmup compare pass before gated measurement, a fixed number of repeated compare runs with median or worst-run acceptance, or an explicit instability note and threshold exclusion rule for rows below a configured absolute-duration floor.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now requires completion-style primary beta runs to include `--warmup --measurement-runs 3`, `stab-bench compare --warmup` runs selected Stab-side workloads once before recording report measurements, repeated recorded measurement runs are aggregated by median, compare reports record `command.warmup` and `command.measurement_runs`, and `just bench::primary-beta` includes `--warmup --measurement-runs 3`.

## 2026-06-28 - M12: Beta Gate Scope For Contract-Only Primary Rows

Status: Resolved
Revealed by: M12 primary compare evidence after reclassifying the M8 primary sampling rows, `m8-sample-high-repeat-contract`, `m9-m2d-bitpacked-contract`, `m9-detect-primary-matrix-contract`, `m9-m2d-primary-matrix-contract`, `m10-analyze-errors-high-repeat-contract`, and four M11 sample_dem rows from `contract-only` to faithful public `stim-cli` baselines.
Current text: M12 said the frozen primary matrix is every benchmark contract row from M4 through M11 except baseline metadata anchors, and completion-style performance runs should pass `--require-beta-gate`, which failed when any selected row lacked a proven Stab-vs-Stim ratio or exceeded the 2.0x beta performance gate.
Gap: the primary matrix still included `m4-circuit-canonical-print`, `m7-convert-stim-canonical`, and `m10-dem-print-contract`, whose best current evidence is Stab-only contract timing because pinned Stim v1.16.0 has no matching public CLI or `stim_perf` baseline for the exact workload.
Proposed amendment: define an M12 beta-gate selection rule that separates comparable primary rows from source-owned contract-representative rows, then require `--require-beta-gate` for every comparable primary row and require each remaining contract-representative row to have either a promoted faithful Stim baseline or an explicit follow-up entry explaining why no ratio can be proven before beta.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now requires comparable rows to pass the active 1.25x beta gate while allowing only measured `contract-only` rows with checked source-owned JSON waivers. `benchmarks/m12-primary-beta-waivers.json` records the current no-ratio rows with reasons and follow-up paths, `stab-bench compare --require-beta-gate --beta-waivers` rejects stale or misapplied waivers, and `just bench::primary-beta` dispatches the completion-style checked run.

## 2026-06-28 - M12: Probability Utility Benchmark Gate Comparability

Status: Resolved
Revealed by: M12 primary compare evidence after optimizing the direct noisy Z-measurement sample path.
Current text: M12 says the beta performance gate applies to every primary parser, generator, sampling, detection, DEM parsing, DEM sampling, and analyzer workload, and the primary matrix includes `m8-probability-util` from `src/stim/util_bot/probability_util.perf.cc`.
Gap: `m8-probability-util` compared Stim's internal `biased_random_1024_*` utility benchmark against a Stab sampler-path contract proxy because Stab did not expose a standalone probability utility API.
Proposed amendment: introduce a Stab probability-draw utility API and a direct benchmark matching Stim's `biased_random_1024_*` filters, while keeping the public sampler probability paths covered by `m8-sample-throughput-*` and statistical oracle rows.
Resolution: Stab now exposes `biased_randomize_bits`, `m8-probability-util` measures seven direct 1024-bit biased-random utility cases against the pinned Stim perf filters, and `target/benchmarks/m12-primary-compare-after-probability-util-direct/compare.json` records the row passing the beta gate at 0.96x.

## 2026-06-27 - M9: Structural Oracle Flag Mismatch

Status: Resolved
Revealed by: running M9 done criteria after implementing the first detection workflow slice.
Current text: M9 lists `just oracle::run --milestone M9 --structural` as a done criterion.
Gap: `stab-oracle run` supported `--exact`, `--statistical`, `--implemented-only`, `--all`, and `--milestone`, but did not support a `--structural` filter; structural implemented rows ran under plain `just oracle::run --milestone M9`.
Proposed amendment: either add a `--structural` filter to `stab-oracle run`, or change milestone done criteria to say that structural rows are checked by `just oracle::run --milestone M9` and exact rows can be checked separately with `--exact`.
Resolution: `stab-oracle run` now supports `--structural`; `just oracle::run --milestone M9 --structural` runs implemented structural rows and reports remaining structural manifest-only rows.

## 2026-06-27 - M5: Memory Test Subcase Granularity

Status: Resolved
Revealed by: full code review of the M5 oracle rows.
Current text: the test-porting plan marked the Memory And Portable SIMD files as P0 for M5 without separating subcases that require APIs not introduced by the M5 portable bit core.
Gap: file-level oracle rows could imply full parity for upstream memory tests that include randomization, shifts, addition, table text parsing, table slicing and resizing, lower-triangular inversion, subset/intersection predicates, and custom allocation/storage utilities.
Proposed amendment: state that M5 owns only the subcases corresponding to the initial Stab bit-core API, and require unsupported upstream subcases to remain deferred until Stab introduces equivalent public or simulator-facing APIs.
Resolution: `docs/plans/stim-test-porting-plan.md` now defines the M5-owned memory subcases, and `oracle/fixtures/manifest.csv` labels implemented M5 memory rows as M5-owned subsets rather than full-file parity.

## 2026-06-27 - M5: Benchmark Compare Semantics

Status: Resolved
Revealed by: milestone audit of the M5 benchmark compare output.
Current text: M5 required `just bench::compare --milestone M5` to report row XOR, matrix transpose, bit-packed copy, sparse XOR, and popcount-like workloads against the M3 baseline.
Gap: the milestone did not distinguish exact upstream workload matches from Stab-only M5 contract-smoke workloads, did not require normalized Stab rates, and did not say whether the current simple matrix transpose helper had to match the upstream 10k optimized transpose benchmark.
Proposed amendment: require M5 compare output to report normalized Stab rates and pinned Stim timings, label non-comparable contract-smoke workloads explicitly, and defer exact optimized 10k bit-table transpose parity to M12 performance hardening.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now names the normalized M5 benchmark evidence and the M12 deferral; `stab-bench compare` prints normalized rates and M5 comparability notes.

## 2026-06-27 - M5: Portable SIMD Feature Gate Location

Status: Resolved
Revealed by: implementation of the M5 portable-SIMD bit kernel.
Current text: M5 said to pin Nightly and isolate `#![feature(portable_simd)]` in bit-kernel modules.
Gap: Rust feature gates are crate-level attributes, so `#![feature(portable_simd)]` cannot be placed only inside a module even when direct `std::simd` imports and operations are module-local.
Proposed amendment: state that the crate-level feature gate is allowed at `stab-core` crate root, while direct `std::simd` imports and operations must stay in approved bit-kernel modules.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now distinguishes the crate-level feature gate from direct SIMD usage.

## 2026-06-27 - M4: Canonical Printer Benchmark Baseline

Status: Resolved
Revealed by: milestone audit of the M4 benchmark evidence.
Current text: M4 required `just bench::compare --milestone M4` to report parser and printer throughput against the M3 C++ baseline, while `m4-circuit-canonical-print` was a contract-only row.
Gap: pinned Stim v1.16.0 has parser and gate lookup perf runners but no direct C++ canonical-printer benchmark runner; using public `stim convert` would benchmark result-format conversion, not `.stim` canonical printing.
Proposed amendment: state that M4 reports parser throughput and gate lookup against the C++ baseline, and reports Stab-only canonical-printer timing against an explicit contract-only printer row without claiming a Stab-vs-Stim printer comparison.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now names the narrower M4 benchmark evidence and the general contract-only benchmark rule.

## 2026-06-27 - M3: Benchmark Compare Acceptance

Status: Resolved
Revealed by: milestone audit of the M3 benchmark harness.
Current text: M3 asks for `just bench::compare` to run Stab and Stim on the same benchmark matrix once Stab supports the feature, but the done criteria only require `bench::baseline`, `bench::list`, and `bench::smoke`.
Gap: the milestone does not define what `bench::compare` must accept, read, report, or fail on before implementation milestones start using it as evidence.
Proposed amendment: require `bench::compare` to read an M3 baseline report or use the documented default, distinguish runnable rows from pending Stab runners, and make `--strict` fail until the owning milestone provides the required Stab runner and complete selected baseline evidence.
Resolution: `stab-bench compare` now reads the default or explicit baseline report, runs Stab comparison runners for supported rows, reports pending rows explicitly, and makes `--strict` fail when any selected row is pending or missing from the selected baseline; `benchmarks/README.md` and `docs/plans/rust-stim-drop-in-rewrite.md` document the behavior.

## 2026-06-27 - M1/M4/M7: CLI Convert Ordering

Status: Resolved
Revealed by: milestone audit of the M1 compatibility matrix and `just oracle::matrix --milestone M4`.
Current text: M1 says planned CLI surfaces are covered in implementation order as `gen`, `convert`, `sample`, `detect`, `m2d`, `analyze_errors`, and `sample_dem`; M4 links `src/stim/cmd/command_convert.test.cc` for parse/canonical-print behavior; M7 tasks say to implement both `stim gen` and `stim convert`.
Gap: the plan does not clearly say whether M4 implements a public `stim convert` subset, only internal parse-print oracle fixtures, or test metadata that M7 later turns into CLI compatibility.
Proposed amendment: state that M4 owns the `.stim` parser/printer library contract and may use `command_convert.test.cc` only as oracle evidence for parse/canonical-print semantics, while M7 owns public `stim convert` CLI compatibility unless the plan explicitly promotes a minimal M4 CLI subset.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now limits M4 benchmarks to parser, printer, and gate lookup, and assigns public `stim convert` CLI compatibility and convert throughput to M7.

## 2026-06-27 - M4/M6/M9: Top-Level Algorithm Fixture Ownership

Status: Resolved
Revealed by: implementation of the M4 oracle rows for circuit, gate, and probability coverage.
Current text: the compatibility matrix and oracle fixture manifest assigned `src/stim/util_top/mbqc_decomposition.test.cc`, `src/stim/util_top/simplified_circuit.test.cc`, and `src/stim/util_top/transform_without_feedback.test.cc` to M4 as `stim-format` rows.
Gap: these upstream tests depend on flow, tableau, simulator, or detector-conversion semantics that M4 does not otherwise own.
Proposed amendment: assign MBQC decomposition and simplified-circuit tests to the tableau milestone and assign transform-without-feedback tests to the detector-conversion milestone.
Resolution: `oracle/compatibility-matrix.csv` and `oracle/fixtures/manifest.csv` now assign MBQC decomposition and simplified-circuit fixtures to M6, and transform-without-feedback fixtures to M9.

## 2026-06-27 - M3: Contract-Only Benchmark Rows

Status: Resolved
Revealed by: implementation of the M3 benchmark manifest.
Current text: M3 requires benchmark contracts for surfaces such as bit-packed `m2d` and `.dem` parse/print while also requiring pinned C++ baseline results.
Gap: some required benchmark contracts do not have a direct `stim_perf` filter or Stim CLI command that exercises the exact future Stab performance surface.
Proposed amendment: allow explicit contract-only benchmark rows when no direct pinned C++ executable baseline exists, require those rows to name their upstream source and owning milestone, and require a runnable benchmark before an implementation milestone claims a Stab-vs-Stim performance comparison.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now makes contract-only benchmark rows explicit in M3.

## 2026-06-27 - M2: Comparator Implementation Ownership

Status: Resolved
Revealed by: milestone audit of the M2 oracle corpus.
Current text: M2 said to define structural and statistical comparators, while later milestones own the first runnable uses of many semantic and statistical comparator families.
Gap: the plan did not say whether M2 must implement every comparator executable or only define comparator contracts and fixture metadata before implementation milestones begin.
Proposed amendment: state that M2 defines comparator contracts and manifest metadata, while the owning M4 through M11 milestones must implement runnable structural or statistical comparator code before marking matching rows `implemented`.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now makes comparator implementation ownership explicit in the M2 task list.

## 2026-06-27 - M8: Linked Simulator And Result-Format Subcase Ownership

Status: Resolved
Revealed by: milestone audit of M8 oracle coverage.
Current text: M8 links the C++ Simulators group for frame, tableau, vector, and graph simulation cases that apply to sampling, and links the C++ Input And Output Formats group for measurement record formats and sparse shots.
Gap: the milestone did not enumerate which upstream simulator subcases are required for the public sampler milestone, which are direct Rust API compatibility tests, and which are later simulator or IO-library work.
Proposed amendment: split M8 acceptance into explicit subcase groups for public `stim sample` CLI parity, result writer byte layouts, result reader/parser APIs, frame/tableau sampling semantics, reference-sample behavior, and simulator-only structural utilities; require every M8-owned group to have runnable fixtures or a named deferred owner before milestone completion.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md`, `oracle/compatibility-matrix.csv`, and `oracle/fixtures/manifest.csv` now scope M8 to frame/tableau sampling semantics, move detection-output helpers to M9, move sparse reverse detector-frame tracking to M10, and move graph/vector simulator internals to M12. The M8 frame and tableau simulator coverage rows are runnable through `cargo test -p stab-core sampling`.

## 2026-06-27 - M8: Benchmark Strictness And Baseline Completeness

Status: Resolved
Revealed by: milestone audit of `just bench::compare --milestone M8`.
Current text: M8 requires `just bench::compare --milestone M8` to report compile/analysis time, single-shot latency, and batch throughput for `1`, `1024`, and `1_000_000` shots.
Gap: non-strict benchmark comparison can exit successfully while M8 benchmark rows have missing pinned Stim baselines, and the milestone does not define which report-only rows are acceptable before completion. Pending Stab runners are no longer part of this gap because every M8 benchmark manifest row now has either a Stab comparison runner or an explicit contract-only runner.
Proposed amendment: require `just bench::compare --milestone M8 --strict` for milestone completion, or explicitly list report-only exceptions with their owning follow-up milestone; every required M8 benchmark row should have a Stab runner and selected pinned Stim baseline before completion.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now requires `just bench::compare --milestone M8 --strict` for M8 completion. Strict comparison validates pinned Stim baseline metadata, rejects unmatched milestone filters, fails invalid placeholder baseline rows, fails empty contract-only placeholders, and the M8 benchmark manifest rows now have Stab runners or measured representative contract rows. Regenerating `target/benchmarks/baseline/latest/baseline.json` with `just bench::baseline --only M8` produced selected pinned Stim rows accepted by the strict comparison.

## 2026-06-27 - M8: Multi-Outcome Statistical Evidence

Status: Resolved
Revealed by: milestone audit of M8 statistical oracle rows.
Current text: M8 requires statistical tests for noisy sampling that do not require C++ random-stream compatibility, and the test strategy names binomial and chi-square checks.
Gap: the milestone does not specify which multi-outcome channels require multinomial or chi-square evidence, what bucket definitions should be used, or what sample counts and false-positive budgets are acceptable for channels such as `PAULI_CHANNEL_2` and heralded local noise.
Proposed amendment: require binomial evidence for one-bit marginal fixtures and chi-square or equivalent multi-bucket evidence for multi-outcome noise fixtures, with fixture-specific bucket definitions, sample counts, fixed seeds, and confidence bounds recorded in the oracle manifest.
Resolution: M8 now includes a bucketed statistical oracle comparator and fixture-specific bucketed rows for `PAULI_CHANNEL_2`, correlated errors, independent X/Y/Z errors, depolarizing basis variants, multi-target `X_ERROR`, and measurement-result flip probabilities. Each row records bucket definitions, sample counts, fixed seed 5, and a 5-sigma tolerance in `oracle/fixtures/manifest.csv`; the oracle harness validates that the declared false-positive budget is not tighter than the tolerance can support.

## 2026-06-27 - M11: Sample Dem CLI Flag And Format Scope

Status: Resolved
Revealed by: milestone audit and full code review against pinned Stim `command_sample_dem.cc` and `dem_sampler.inl`.
Current text: M11 requires `stim sample_dem` with supported flags, detector output, observable output, bit-packed formats, seed handling, and deterministic behavior where applicable.
Gap: the milestone does not enumerate the exact Stim v1.16.0 `sample_dem` flag set, and therefore does not say whether `--err_out`, `--err_out_format`, `--replay_err_in`, `--replay_err_in_format`, `ptb64` detector/observable/error streams, or Stab-only observable append/prepend flags are in scope for the initial M11 completion bar.
Proposed amendment: list the required M11 public `sample_dem` flags and formats explicitly. If full Stim parity is required in M11, require independent detector, observable, and error streams; error recording and replay; `01`, `b8`, `r8`, `ptb64`, `hits`, and `dets` where upstream accepts them; and oracle rows for each stream route. If the initial milestone intentionally excludes some of these surfaces, add explicit deferrals with compatibility-matrix rows and require unsupported flags to fail with clear errors.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now lists the required M11 `sample_dem` flags as `--shots`, `--in`, `--out`, `--out_format`, `--seed`, `--obs_out`, `--obs_out_format`, `--err_out`, `--err_out_format`, `--replay_err_in`, and `--replay_err_in_format`; it requires `01`, `b8`, `r8`, `ptb64`, `hits`, and `dets` for detector, observable, error, and replay streams where Stim accepts those formats. Stab-only `--append_observables` and hidden `--prepend_observables` are explicitly excluded from M11 Stim parity evidence and must reject conflicts if retained.

## 2026-06-27 - M11: DEM Sampler Fixture Group Acceptance

Status: Resolved
Revealed by: milestone audit of the M11 oracle manifest, direct Rust tests, and benchmark rows.
Current text: M11 says to add sparse, dense, repeated, and high-detector-count DEM fixture groups, and the done criteria require exact, statistical, structural, and benchmark checks.
Gap: the milestone does not define which fixture groups must be oracle rows, which can be direct Rust tests, which can be benchmark-only representatives, what comparator each group uses, or what sample counts and statistical bounds prove noisy sparse, dense, repeated, high-detector, observable-only, and correlated-error behavior.
Proposed amendment: define an M11 fixture matrix with rows for deterministic exact output, statistical noisy sampling, sparse detector ids, dense detector targets, repeated detector shifts, high detector ids, observable-only errors, detector-observable correlation, and correlated detector combinations. Each row should name its upstream source, comparator mode, sample count or structural assertion, output format, and whether it is acceptance evidence or benchmark-only evidence.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now defines the M11 fixture acceptance matrix. The matrix requires exact and statistical evidence for basic, sparse, dense, repeated, high-detector, observable-only, detector-observable correlation, and correlated detector-combination groups, exact side-output oracle rows for observable, error, and replay side streams, and a direct Rust structural row for the M11-owned `dem_sampler` subset.

## 2026-06-27 - M11: DEM Sampler Streaming And Scale Limits

Status: Resolved
Revealed by: full code review of `CompiledDemSampler` and `stab sample_dem` resource behavior.
Current text: M11's objective says to implement fast DEM-based sampling, and the tasks require reusable analysis state, per-shot sampling, repeated DEM fixtures, high-detector fixtures, and bit-packed formats.
Gap: the milestone does not specify whether M11 must stream shots like Stim's striped sampler, what maximum supported `--shots`, detector count, observable count, error count, DEM input size, or output byte count is acceptable during the initial implementation, or whether bounded repeat unrolling is an accepted temporary design.
Proposed amendment: require a compiled or streaming DEM sampler API that can write output in bounded chunks without materializing all shots, or explicitly document initial resource limits and add rejection tests for excessive shots, excessive detector/observable widths, excessive error counts, oversized DEM input, and nested repeat expansion. State whether folded repeat sampling is an M11 requirement or an M12 performance-hardening task.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now accepts a bounded materialized M11 sampler and names the required limits. The implementation adds `CompiledDemSampler::validate_sample_buffer_units`, a 64 MiB `sample_dem` DEM input cap, core plus CLI rejection tests for excessive shot counts, high detector widths, optional generated or replayed error-record buffers, and bounded repeat expansion. Replay input reads only the requested `ptb64`, `b8`, and `r8` record prefix, text replay records are capped at 1,048,576 bytes per requested record, and extra replay records after `--shots` are ignored. True streaming output, folded repeat sampling without bounded unrolling, exact output-byte budgeting, and performance thresholds are deferred to M12.

## 2026-06-27 - M11: Benchmark Baseline And Comparability

Status: Resolved
Revealed by: milestone audit and full code review of `just bench::compare --milestone M11`.
Current text: M11 requires `just bench::compare --milestone M11` to report sparse, dense, repeated, and high-detector-count DEM sampling throughput.
Gap: the milestone does not say whether M11 completion requires a selected pinned-Stim baseline artifact, strict comparison, external `stab-cli sample_dem` process timings, in-process Stab core timings, or report-only representative workloads. The current Stab runners print useful in-process rates, but the latest local baseline artifact can omit M11 pinned-Stim rows and the `stim-cli` row is not an external CLI-vs-CLI comparison.
Proposed amendment: define M11 benchmark acceptance as either `just bench::compare --milestone M11 --strict` against a baseline that includes `m11-dem-sampler` and `m11-sample-dem-cli`, or explicitly label the M11 benchmark rows as report-only until M12. If CLI comparability is required, add a Stab subprocess runner using the same argv and stdin path as the Stim CLI baseline, and normalize rates by shots, detector bits, error operations, and output bytes where appropriate.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now defines M11 benchmark acceptance as report-only Stab-side throughput from `just bench::compare --milestone M11`. Strict pinned-Stim baseline completeness, external CLI-vs-CLI process timing comparability, performance thresholds, and normalized primary-matrix reporting are M12 responsibilities.

## 2026-06-27 - M9: Feedback-Removal Conversion Scope

Status: Resolved
Revealed by: implementing `stab m2d` and inspecting pinned Stim `command_m2d.test.cc` plus `transform_without_feedback.test.cc`.
Current text: M9 requires `stim m2d` with measurement input parsing, detector conversion, observable output, and inconsistent-input errors, and the compatibility matrix assigns `transform_without_feedback.test.cc` to M9.
Gap: the milestone does not explicitly say whether `m2d --ran_without_feedback` and circuit feedback inlining are required for the initial M9 CLI surface, even though pinned Stim tests exercise that path and Stab currently rejects the flag instead of silently returning incorrect output.
Proposed amendment: add an explicit M9 task and fixture group for `--ran_without_feedback` if feedback-removal parity is required now, or defer it to a named later detector-conversion submilestone while requiring the CLI to reject the flag with a clear error.
Resolution: the later M9 sweep and detector-utility follow-ups implemented the scoped `m2d --ran_without_feedback` path and promoted `coverage-util-top-transform-without-feedback` into an executable row for the owned subset. The implementation covers basic measurement feedback, demolition feedback, interleaved ordering, sweep-control preservation, and MPP feedback inlining while rejecting repeat blocks and unsupported classical controlled feedback gates. Exact loop refolding and full feedback-transform parity remain future work.

## 2026-06-27 - M9: Detector Analysis Utility Row Ownership

Status: Resolved
Revealed by: M9 oracle manifest after implementing detector sampling, measurement-to-detection conversion, observable output, and M9 benchmark runners.
Current text: M9 links detector-conversion workflows and the compatibility matrix assigns `circuit_to_detecting_regions.test.cc`, `missing_detectors.test.cc`, and `transform_without_feedback.test.cc` to M9.
Gap: the milestone objective and tasks describe public `detect` and `m2d` workflows, but they do not define public Rust APIs, fixture subsets, or done criteria for detecting regions, missing-detector analysis, or feedback-removal transforms; those rows remain manifest-only while the public CLI/core conversion rows are implemented.
Proposed amendment: split M9 into explicit public workflow acceptance and detector-analysis utility acceptance, naming the APIs and upstream subcases required for each utility row, or move the utility rows to the DEM/analyzer milestone that introduces the required analysis structures.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now splits M9 public workflow acceptance from detector-analysis utilities. The M9 detector utility closure promoted simple detecting regions, basic single-record missing-detector suggestions, and MPP feedback inlining into explicit Rust APIs and executable rows. PF5 later promoted detecting-region repeat traversal, detector and logical-observable target filters, generated repetition-code all-target/all-tick selection with selected exact D0, D6, and L0 regions, fixed single-qubit and two-qubit Clifford propagation for plain qubit targets, ignored anticommutation mode, missing-detectors Gaussian row reduction, repeated MPP and pair-measurement stabilizer products, record-only observable rows, ignored Pauli observable rows, tableau-backed Clifford propagation for plain qubit target groups, bounded repeat traversal with explicit expansion caps, and the pinned honeycomb and toric generated-code suffix cases. Broader detecting-region target-shape support, broader generated-code detecting-region extraction, gauge handling, broader generated-code missing-detector suffix analysis, folded large-repeat traversal beyond current caps, exact loop refolding, public measurement-rich flow solving, and full transform API parity remain future work with explicit manifest or gap entries.

## 2026-06-27 - M9: Sweep-Conditioned Detection Conversion Scope

Status: Resolved
Revealed by: milestone audit against pinned Stim `measurements_to_detection_events.test.cc`.
Current text: M9 requires measurement-to-detection conversion from measurement records and circuits with detectors, observables, coordinate shifts, and repeats, but it does not mention sweep data or sweep-conditioned detector expectations.
Gap: upstream measurement-to-detection tests include sweep-bit inputs that can alter expected detector parities through sweep-controlled operations, while the current Stab converter and `m2d` CLI accept only measurements, a circuit, and reference-sample options.
Proposed amendment: state whether sweep-conditioned conversion is in M9 scope. If it is, require typed sweep inputs, `--sweep` and `--sweep_format` CLI flags, and fixtures for sweep count mismatches and sweep-controlled parity changes. If not, move those upstream subcases to the first milestone that introduces sweep-aware simulation.
Resolution: M9 now explicitly excludes sweep input data for detection conversion. `detect`, `m2d`, and core detection conversion reject sweep-conditioned circuits with a clear error until a later sweep-aware simulation milestone introduces typed sweep inputs and CLI flags. Evidence is `cargo test -p stab-core detection_conversion_rejects_sweep_conditioned_circuits_until_sweep_inputs_exist --quiet`, `cargo test -p stab-cli m2d_rejects_sweep_conditioned_conversion_until_sweep_inputs_exist --quiet`, and the updated `coverage-simulators-measurements-to-detection-events-rust` manifest row.

## 2026-06-27 - M9: Benchmark Baseline Completeness

Status: Resolved
Revealed by: milestone audit of `just bench::compare --milestone M9` and `just bench::compare --milestone M9 --strict`.
Current text: M9 requires `just bench::compare --milestone M9` to report `detect` and `m2d` throughput separately for text and bit-packed formats, while the benchmark plan describes comparisons against pinned Stim v1.16.0.
Gap: the non-strict compare command reports Stab-side M9 timings, but the current baseline artifact has no M9 pinned Stim rows, so `--strict` fails and the command is not a complete Stab-vs-Stim comparison.
Proposed amendment: either require M9 to record selected pinned Stim detect and m2d baselines before completion and run the strict comparison, or label M9 benchmark evidence as report-only until M12 freezes the primary performance matrix.
Resolution: M9 benchmark acceptance is explicitly report-only Stab-side timing from `just bench::compare --milestone M9`. Strict pinned-Stim baseline completeness, external CLI-vs-CLI timing comparability, beta-gate ratios, and promoted primary-matrix baseline rows belong to M12, where selected M9 rows can gain faithful public Stim CLI baselines without changing M9 completion. Evidence is `benchmarks/manifest.csv` marking the M9 rows as `report-only`, `cargo test -p stab-bench m9_benchmark_rows_have_stab_compare_runners --quiet`, and the M12 progress note for promoted M9 baseline rows.

## 2026-06-27 - M9: Detection Bit-Packed Format Scope

Status: Resolved
Revealed by: milestone audit against pinned Stim `command_detect.cc`, `command_m2d.cc`, and `measurements_to_detection_events.test.cc`.
Current text: M9 requires `stim detect` with bit-packed modes and `stim m2d` with measurement input parsing, while the benchmark plan names text and bit-packed input.
Gap: the milestone does not say whether M9 bit-packed parity means the `b8` subset needed by current decoder workflows or every Stim v1.16.0 bit-packed format including `ptb64`, nor does it name zero-width bit-packed input behavior as an acceptance case.
Proposed amendment: define the exact M9 bit-packed parity boundary by command and stream: `b8` for public detector and observable streams, `ptb64` for `detect` detector output and `detect --obs_out`, and `ptb64` for `m2d` measurement input only. Require `m2d --out_format=ptb64` and `m2d --obs_out_format=ptb64` to reject like pinned Stim v1.16.0, require zero-width `ptb64` input rejection, and require decoded-record bounds before allocation.
Resolution: M9 now requires `b8` parity for public `detect` and `m2d` detector and observable streams, `ptb64` parity for `detect` detector output, `detect --obs_out`, and `m2d` measurement input, plus explicit rejection for `m2d` `ptb64` detector and observable outputs. Evidence is `cargo test -p stab-cli m9`, including `detect_writes_ptb64_detector_and_observable_outputs`, `detect_rejects_ptb64_shots_that_are_not_multiple_of_64`, `m2d_reads_ptb64_records_and_writes_supported_formats`, `m2d_rejects_ptb64_detector_output_like_stim`, `m2d_rejects_ptb64_observable_output_like_stim`, `m2d_rejects_zero_width_ptb64_input`, and `m2d_rejects_excessive_ptb64_decoded_shots_before_expansion`, plus `cargo test -p stab-core result_formats::tests::ptb64_records_are_measurement_major_over_64_shot_groups`.

## 2026-06-27 - M9: Generated Fixture Round-Trip Coverage

Status: Resolved
Revealed by: milestone audit comparing the M9 task list to current oracle and benchmark evidence.
Current text: M9 says to add round-trip tests for bit-packed input/output and text input/output across circuit fixtures generated in M7.
Gap: current M9 exact oracle rows use hand-authored circuits and measurement records, while generated repetition-code coverage exists in benchmark runners instead of runnable oracle or test acceptance rows; the plan does not define the generated fixture matrix, output formats, round-trip direction, or whether benchmark primary-matrix representatives count as acceptance evidence.
Proposed amendment: add explicit generated-fixture M9 oracle or Rust tests for selected M7 repetition, rotated surface, unrotated surface, and color-code circuits across `01`, `dets`, and `b8` conversion paths, or narrow the task to say generated-fixture coverage is benchmark evidence only until the primary matrix is frozen.
Resolution: M9 now treats generated-fixture acceptance as `sample -> m2d` public-workflow round trips compared with `detect` for M7 repetition, rotated-surface, unrotated-surface, and color-code circuits in `01` text and `b8` bit-packed output with appended observables. Evidence is `cargo test -p stab-cli m2d_round_trips_generated_m7_circuits_in_text_and_bitpacked_formats --quiet` and the oracle manifest row `coverage-simulators-measurements-to-detection-events-generated`. Existing hand-authored M9 oracle rows continue to cover `dets` label formatting.

## 2026-06-27 - M9: Pauli-Target Observable Detection Scope

Status: Resolved
Revealed by: full code review against pinned Stim `frame_simulator` observable handling.
Current text: M9 requires `stim detect` with observables and detector output handling.
Gap: the milestone did not distinguish measurement-record observables from `OBSERVABLE_INCLUDE` Pauli target observables. The prior implementation rejected Pauli-target observables for `detect` to avoid silently returning incorrect logical flips, while `m2d` continued to ignore Pauli targets like pinned Stim's measurement-to-detection converter.
Proposed amendment: either require M9 to implement frame-simulator-style Pauli-target observable flips for `detect`, including deterministic and random observable fixtures, or defer Pauli-target observable detection to the simulator-completeness milestone while requiring an explicit error in the M9 CLI and Rust API.
Resolution: M9 now requires `stim detect` to implement frame-simulator-style Pauli-target observable flips for the documented scalar frame subset while leaving `m2d` conversion behavior unchanged. Evidence is `coverage-simulators-frame-simulator-pauli-observables`, `cargo test -p stab-core detection_sampling`, including RX/RY/RZ Pauli observable parity, product-measurement frame updates, and reference-sample measurement-bit cancellation, plus `cargo test -p stab-cli detect_supports_pauli_target_observable_flips`, `cargo test -p stab-cli detect_supports_product_measurements_with_pauli_observable_flips`, and `cargo test -p stab-cli m2d_ignores_pauli_target_observables_like_stim_conversion`.

## 2026-06-27 - M9: Detection Conversion Streaming And Scale Limits

Status: Resolved
Revealed by: full code review of `detect` and `m2d` resource behavior.
Current text: M9 requires decoder-pipeline detection workflows and benchmark reporting but does not define streaming, batching, loop-folding, or maximum supported record sizes.
Gap: current Stab materializes measurement records and detection records in memory and unrolls detection-conversion repeats within explicit temporary limits. This prevents unbounded CPU or memory use for hostile inputs, but it is not a final decoder-scale streaming design and does not match Stim's ability to process large files and folded repeats efficiently.
Proposed amendment: add a follow-up milestone or M12 task for compiled/streaming detection conversion that processes records in bounded batches, preserves repeat structure where possible, avoids duplicate sampler analysis, documents or removes temporary limits, and includes benchmark rows for large generated-code detector workloads.
Resolution: M9 now explicitly accepts a bounded materialized detection-conversion implementation with documented temporary limits: 1,000,000 bits for measurement, detector, and observable record widths, 64,000,000 buffered bits for materialized measurement samples and detection records, and 100,000 repeat iterations during conversion planning. M12 now owns compiled or streaming detection conversion when benchmark evidence shows the materialized path is the bottleneck, including bounded batches, folded-repeat preservation where possible, duplicate-analysis removal, and large generated-code `detect` and `m2d` benchmark rows. Evidence is `cargo test -p stab-core detection_conversion_rejects_unbounded_record_shapes --quiet` and the M12 task list in `docs/plans/rust-stim-drop-in-rewrite.md`.

## 2026-06-27 - M8: Skip Loop Folding Scope

Status: Resolved
Revealed by: milestone audit of M8 `--skip_loop_folding` evidence.
Current text: M8 requires repeat handling, reference sample behavior, and `stim sample` core flags, but does not say whether `--skip_loop_folding` must change the Rust sampler implementation or only be accepted with output-compatible behavior.
Gap: Stab currently accepts `--skip_loop_folding` and proves output parity on a repeat circuit, while optimized loop-folded reference-sample construction remains deferred by the `coverage-util-top-reference-sample-tree` manifest row. The milestone text does not state whether that is sufficient for M8 completion.
Proposed amendment: state that M8 requires `--skip_loop_folding` to be accepted and output-compatible for repeat circuits, while optimized loop-folded reference-sample construction and performance parity are deferred to M12 unless promoted earlier.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now defines `--skip_loop_folding` acceptance as output-compatible repeat-circuit behavior for M8 and defers optimized loop-folded reference-sample construction plus performance parity to M12. Evidence is the implemented oracle fixture `m8-sample-skip-loop-folding` and the structural row `coverage-util-top-reference-sample-tree`, whose manifest note records the optimized construction deferral.

## 2026-06-27 - M7: Generated Fixture Matrix Scope

Status: Resolved
Revealed by: milestone audit of M7 generator oracle rows and structural generator tests.
Current text: M7 says to store generated circuit fixture matrices by family, task, distance, rounds, and noise settings for later M8 through M12 reuse, and says `stab-cli gen` output must match Stim v1.16.0 for the compatibility matrix of families, tasks, distances, rounds, and noise settings.
Gap: the milestone does not define the concrete matrix dimensions, required noise settings, fixture artifact format, acceptable storage size, whether every matrix point needs exact CLI golden output or direct Rust structural parity, or how the matrix is reused by later milestones without checking in very large circuit outputs.
Proposed amendment: define a primary M7 generator matrix with explicit family, task, distance, round, and noise tuples; require exact CLI goldens only for a small public-command subset and direct Rust structural or generated-on-demand oracle checks for the larger matrix; name the fixture artifact location and the later milestones that consume each fixture group.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now defines the M7 generated-circuit acceptance matrix as source-owned oracle, direct-test, and benchmark manifests instead of checked-in generated circuit bodies. Exact CLI goldens cover the public command shape for supported families and tasks, direct Rust structural tests cover representative larger noisy family/task/distance/round/probability cases, and benchmark rows cover generated-on-demand primary matrix circuits reused by M8 through M12.

## 2026-06-27 - M7: Convert Command Circuit Versus Result-Format Scope

Status: Resolved
Revealed by: implementation and upstream test inspection of `src/stim/cmd/command_convert.test.cc`.
Current text: M7 requires `stim convert` for `.stim` parse and canonical print workflows and links `command_convert.test.cc` as a direct CLI command test source.
Gap: pinned Stim v1.16.0 `command_convert.test.cc` primarily tests measurement, detector, and observable result-format conversion among formats such as `01`, `b8`, `hits`, `r8`, and `dets`, often with `--circuit`, `--dem`, `--types`, and observable-output routing, while `.stim` canonical circuit parse-print behavior is already owned by the M4 core parser/printer fixtures and is not an exact upstream `stim convert` command surface.
Proposed amendment: split M7 convert acceptance into two explicit tracks: a Stab-specific `convert --in_format=stim --out_format=stim` canonical circuit workflow backed by M4 parser/printer tests, and pinned-Stim-compatible result-data conversion rows backed by `command_convert.test.cc`; defer full `b8`, `hits`, `r8`, `--circuit`, `--dem`, `--types`, and `--obs_out` support to the first milestone that owns the corresponding measurement-record and detector-error-model APIs if M7 does not introduce those APIs.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now splits M7 convert acceptance into `convert --in_format=stim --out_format=stim` canonical circuit workflows backed by M4 parser/printer behavior and pinned-Stim result-data conversion rows backed by `command_convert.test.cc`. `oracle/fixtures/manifest.csv` includes a Stim-compatible rejection row for `--bits_per_shot` to `dets`, and `README.md` documents supported and deferred conversion behavior.

## 2026-06-27 - M6: Random Generation Hook Ownership

Status: Resolved
Revealed by: milestone audit of the M6 stabilizer algebra implementation and benchmark rows.
Current text: M6 requires random generation hooks and links upstream `tableau_random*`, Clifford random distribution, and stabilizers-to-tableau fuzz and perf coverage.
Gap: the milestone does not define which Rust RNG type, seeding contract, distribution parity, or public random-constructor API must exist before Stab has simulator and sampling consumers.
Proposed amendment: state that M6 must either introduce explicit deterministic random hooks for `CliffordString`, `PauliString`, and `Tableau` with documented seed and distribution contracts, or defer random generation to the first simulator/sampler milestone that consumes those hooks while keeping M6 deterministic algebra and iterator coverage.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now defines caller-owned `rand::Rng` hooks for `PauliString`, `SingleQubitClifford`, `CliffordString`, and `Tableau`. Seeded Rust RNGs give reproducible Stab output and exact Stim C++ random-stream parity is not required. `PauliString` samples uniformly over sign and `I`/`X`/`Y`/`Z` bases, including zero-length sign sampling; `CliffordString` samples uniformly over the 24 single-qubit Clifford gates; `Tableau::random` samples valid Clifford tableaus from random Clifford-circuit shapes; and exact uniform tableau sampling or random-workload performance parity is deferred to M12 if needed.

## 2026-06-27 - M6: Util-Top Algorithm Subset Boundaries

Status: Resolved
Revealed by: milestone audit of M6 `circuit_flow_generators`, `has_flow`, `circuit_inverse_qec`, `simplified_circuit`, `mbqc_decomposition`, `circuit_vs_tableau`, and `stabilizers_to_tableau` rows.
Current text: M6 links related util-top tests when their dependencies are in scope, but the oracle manifest records several rows as implemented with notes that defer measurement-rich, detector, noise, sampled-flow, full-gate, tableau-to-circuit, and fuzz variants.
Gap: the milestone does not split deterministic unitary/tableau subset parity from full upstream util-top parity, so an implemented row can be misread as full Stim parity for the entire upstream file.
Proposed amendment: split each related util-top row into explicit subcases owned by M6 and deferred subcases owned by the simulator, detector, or performance-hardening milestones; require public APIs for subset helpers to document unsupported semantics until the deferred rows are implemented.
Resolution: M6 now splits util-top ownership by manifest row. The roadmap names the deterministic unitary and tableau-backed subcases owned by `coverage-util-top-circuit-flow-generators`, `coverage-util-top-has-flow`, `coverage-util-top-circuit-inverse-qec`, `coverage-util-top-circuit-vs-tableau`, `coverage-util-top-simplified-circuit`, `coverage-util-top-mbqc-decomposition`, and `coverage-util-top-stabilizers-to-tableau`, and it names deferred measurement-rich, detector, noise, sampled-flow, full-gate, tableau-to-circuit, and unsupported-semantics variants. Evidence is `just oracle::list --milestone M6`, `just oracle::run --milestone M6 --structural`, and the implemented `coverage-util-top-*` manifest notes.

## 2026-06-27 - M7: Generator Benchmark Comparability

Status: Resolved
Revealed by: implementation of Stab-side M7 benchmark runners for `just bench::compare --milestone M7 --strict`.
Current text: M7 requires generator throughput for repetition, rotated surface, unrotated surface, and color code circuits, and the benchmark manifest uses pinned Stim CLI rows for `stim gen` plus a `main_sample*` CLI dispatch perf row.
Gap: the plan does not specify whether Stab-side generator benchmark evidence must measure direct Rust generator construction, `stab-cli gen` end-to-end execution, canonical `.stim` printing cost, process startup cost, or all of these separately.
Proposed amendment: split M7 benchmark acceptance into explicit rows for direct Rust generator construction, `stab-cli gen` in-process dispatch, and external process startup or canonical text emission if those are required; keep the current Stab direct generator rows report-only until an exact CLI-vs-CLI threshold is specified.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now defines M7 benchmark acceptance as report-only direct Rust generator construction, in-process CLI dispatch, and canonical conversion timings. Strict external CLI-vs-CLI thresholds and process-startup comparability are deferred to M12 performance hardening.

## 2026-06-27 - M6: Stabilizers Versus Amplitudes Dependency

Status: Resolved
Revealed by: milestone audit of the M6 linked-test list and compatibility matrix.
Current text: M6 lists `stabilizers_vs_amplitudes` as a related util-top test when dependencies are in scope.
Gap: the plan does not say which amplitude-state or simulator dependency brings this row into scope, and no M6 fixture manifest row currently names the semantic subset that should be proven by the algebra milestone alone.
Proposed amendment: either add a deterministic algebra-only fixture for the subcases that can be checked without an amplitude simulator, or move `stabilizers_vs_amplitudes` to the tableau simulator milestone with a clear dependency note.
Resolution: M6 now owns the deterministic algebra-only `unitary_to_tableau` subset from `stabilizers_vs_amplitudes`, covering all 46 canonical known-unitary gate-data matrices, controlled-gate endian mapping, Stim-style phase smoothing, and malformed or non-Clifford matrix rejection via `coverage-util-top-stabilizers-vs-amplitudes`. `tableau_to_unitary`, random tableau/unitary roundtrips, and amplitude-simulator cross-checks are explicitly deferred.

## 2026-06-28 - M6: Stabilizers Vs Amplitudes Gate-Data Breadth

Status: Resolved
Revealed by: milestone audit of the `coverage-util-top-stabilizers-vs-amplitudes` fixture after implementing `unitary_to_tableau`.
Current text: M6 tracks a selected `unitary_to_tableau` subset from `stabilizers_vs_amplitudes` and the fixture manifest marks that selected subset as implemented.
Gap: upstream Stim's `unitary_to_tableau_vs_gate_data` test iterates every gate with a known unitary matrix, but Stab currently proves only selected single-qubit matrices plus the four upstream endian examples. The plan does not yet assign the exhaustive known-unitary gate-data matrix sweep to a specific milestone or manifest row.
Proposed amendment: add a separate compatibility row for exhaustive known-unitary gate matrix coverage once Stab has centralized gate-unitary data, or explicitly defer that sweep to the matrix/state-vector milestone that also owns `tableau_to_unitary`.
Resolution: `coverage-util-top-stabilizers-vs-amplitudes` now covers the upstream known-unitary gate-data loop directly with 24 canonical single-qubit matrices and 22 canonical paired-gate matrices copied from pinned Stim v1.16.0 gate data, plus a count check tied to that scope. The plan and manifest now treat exhaustive `unitary_to_tableau` gate-data coverage as M6 evidence; only `tableau_to_unitary`, random roundtrips, and amplitude-simulator checks remain deferred.

## 2026-06-27 - M6: Stabilizer Benchmark Exact Workload Parity

Status: Resolved
Revealed by: milestone audit of `just bench::compare --milestone M6`.
Current text: M6 requires `just bench::compare --milestone M6` to report Pauli, Clifford, tableau, tableau-iterator, and stabilizers-to-tableau workloads, while benchmark manifest rows point at upstream random, fuzz-like, and large-tableau perf filters.
Gap: the milestone does not distinguish report-only deterministic Stab benchmark runners from exact parity with upstream random and 10K-qubit perf workloads.
Proposed amendment: require M6 compare output to provide deterministic Stab-side timings and normalized rates for each M6 benchmark row, label non-exact benchmark workloads in compare notes, and defer exact random and large-tableau threshold parity to M12 performance hardening after random hooks and optimized tableau internals are specified.
Resolution: M6 benchmark acceptance is now explicitly report-only deterministic Stab-side timing from `just bench::compare --milestone M6`. The roadmap allows direct operation-shape matches for Pauli, Clifford, and Pauli-iterator rows only when compare notes say so, while tableau, tableau-iterator, and stabilizers-to-tableau workloads remain deterministic substitutes until M12 decides exact random, fuzz-like, signed-tableau, and 10K-qubit threshold parity. Evidence is `cargo test -p stab-bench m6_benchmark_rows_have_stab_compare_runners --quiet` and `just bench::compare --milestone M6`.

## 2026-06-27 - M6: Stabilizer Algebra Public View And Text Scope

Status: Resolved
Revealed by: implementation of the first owned Pauli-string algebra slice and upstream stabilizer scan.
Current text: M6 requires `PauliString`, `CliffordString`, `Tableau`, related iterators or views, sign handling, and text round trips.
Gap: the milestone does not say whether Rust must expose a public borrowed `PauliStringRef` equivalent, does not distinguish real-phase C++ `PauliString` text from phase-general `FlexPauliString` sparse and lowercase text, and does not define which Python-facing phase semantics are required before the Python API milestone.
Proposed amendment: state that M6 starts with owned Pauli, FlexPauli, Clifford, and Tableau APIs; borrowed views may stay internal unless a later M6 task proves a public view is necessary; text parity must separately cover real dense `PauliString` syntax and phase-general `FlexPauliString` dense or sparse syntax; Python-only binding behavior is semantic-mining input but not a public API requirement until the Python milestone.
Resolution: M6 now states that the public Rust API starts with owned `PauliString`, `FlexPauliString`, `CliffordString`, and `Tableau` values, public `PauliStringRef` parity is not required unless later parity or performance work proves it necessary, and Python-only binding behavior remains semantic-mining input until the Python API milestone. `crates/stab-core/tests/stabilizers.rs` now checks that real `PauliString` rejects imaginary, lowercase, and sparse-style text while `FlexPauliString` accepts phase-general dense and sparse text with canonical display; manifest rows `coverage-stabilizers-pauli-string`, `coverage-stabilizers-flex-pauli-string`, and `coverage-stabilizers-pauli-string-ref` track the split.

## 2026-06-27 - M4: Gate Decomposition Utility Scope

Status: Resolved
Revealed by: implementation of `coverage-circuit-gate-decomposition` as a direct Rust oracle row.
Current text: M4 links `src/stim/circuit/gate_decomposition.test.cc` under Circuit Model, Parser, Targets, And Decomposition, but M4's objective is the public `.stim` data model, gate metadata, parser, validator, and canonical printer.
Gap: the upstream file mixes pure circuit-structure helpers, such as target grouping and disjoint segmentation, with semantic MPP/SPP decomposition behavior that later depends on base-gate decomposition, flows, tableaus, and simulator correctness.
Proposed amendment: state that M4 owns structural decomposition prerequisites only, including Pauli-product grouping and disjoint target segmentation; full `decomposed` behavior for MPP, SPP, pair measurements, and base-gate lowering should move to the first milestone that implements the required tableau/simulator semantics or receive its own explicit milestone task.
Resolution: M4 now explicitly owns only structural decomposition prerequisites through `coverage-circuit-gate-decomposition`: target grouping and disjoint segmentation. Full semantic `decomposed` behavior for MPP, SPP, pair measurements, base-gate lowering, and tableau or simulator equivalence is assigned to the first milestone with the required algebra, flow, simulator, or analyzer semantics, including the M6 util-top rows and later detector/analyzer milestones. Evidence is `cargo test -p stab-core gate_decomposition --quiet` and the implemented manifest row.

## 2026-06-27 - M4: Probability Utility Fixture Scope

Status: Resolved
Revealed by: implementation of `coverage-util-bot-probability-util` as a direct Rust oracle row.
Current text: M4 requires gate argument rules and probability validation, while the test-porting plan points at `src/stim/util_bot/probability_util.test.cc` for probability validation.
Gap: the referenced upstream file also tests `sample_hit_indices` and biased random bit generation, which require RNG and bit-storage behavior that M4 does not otherwise define.
Proposed amendment: state that M4 owns only closed-unit probability validation and disjoint probability-list validation from this file; random hit-index sampling and biased bit generation should move to the first milestone that introduces equivalent RNG and bit/sampler APIs.
Resolution: M4 now owns only closed-unit probability validation and disjoint probability-list validation from `src/stim/util_bot/probability_util.test.cc`. Random hit-index sampling and biased random bit generation are excluded from M4 acceptance and are assigned to the first bit or sampler milestone that consumes equivalent APIs, plus M12 performance hardening when those utilities become benchmark targets. Evidence is `cargo test -p stab-core probability --quiet` and the implemented `coverage-util-bot-probability-util` manifest row.

## 2026-06-27 - M2: Manifest-Only Subcase Granularity

Status: Resolved
Revealed by: milestone audit of the M2 manifest coverage rows.
Current text: M2 and the test-porting plan allow red or manifest-only oracle cases for all P0 and P1 files needed by M4 through M11.
Gap: file-level manifest-only rows can satisfy coverage without identifying the upstream subcases, fixture families, malformed-input cases, or extraction criteria that future implementation milestones must port.
Proposed amendment: require manifest-only rows to name planned subcase groups or extraction criteria for each upstream test file before the owning implementation milestone starts.
Resolution: M2 now requires every manifest-only row to identify planned subcase groups, fixture families, malformed-input classes, or extraction criteria in its manifest note, and file-level placeholders must be split or updated before the owning implementation milestone starts. The remaining manifest-only M9 detector-analysis rows now name their planned subcase groups for detecting regions, missing detectors, and feedback inlining. Evidence is `rg ",manifest-only," oracle/fixtures/manifest.csv` and `just oracle::list --milestone M9`.

## 2026-06-26 - M0: Upstream Smoke References Overreach

Status: Resolved
Revealed by: milestone audit of the M0 oracle lab implementation.
Current text: M0 links `src/stim.test.cc`, `src/stim/main_namespaced.test.cc`, and `src/stim_included_twice.test.cc` as C++ smoke references.
Gap: those upstream files include behavior from later milestones, including circuit parsing, gate metadata, analyzer behavior, and richer CLI mode handling, so treating the full files as M0 requirements would pull M4, M6, and M10 work into the foundation milestone.
Proposed amendment: clarify that M0 extracts only oracle-process smoke checks from these files, specifically help-command health, main binary namespacing health, and one tiny deterministic circuit case; all parser, gate table, analyzer, and broader CLI behavior stays with later milestones.
Resolution: M0 now extracts only oracle-process smoke checks from the upstream smoke references: help-command health, binary namespacing or inclusion health, and one tiny deterministic circuit case. Full parser behavior, gate metadata, analyzer behavior, and broader CLI mode handling stay with their owning implementation milestones. Evidence is `just oracle::run --case smoke/help`, `just oracle::run --case smoke/tiny-circuit`, and the M0 linked-test text in `docs/plans/rust-stim-drop-in-rewrite.md`.

## 2026-06-26 - M0: Oracle Tiny Sample Shim Boundary

Status: Resolved
Revealed by: milestone audit and full-code-review of the M0 `stab-cli sample` smoke shim.
Current text: M0 requires `just oracle::run --case smoke/tiny-circuit`, while the CLI compatibility order defers real `sample` support to M8.
Gap: the plan does not say whether a minimal M0 sample command counts as CLI compatibility or is only an oracle fixture target.
Proposed amendment: state that any M0 sample path is an oracle-only smoke shim and does not count as implemented `stim sample` compatibility; M8 remains responsible for the public `sample` command contract.
Resolution: The M0 roadmap now states that any M0 `sample` path is an oracle-only smoke shim for `smoke-tiny-circuit` and does not count as public `stim sample` CLI compatibility. M8 remains responsible for the real `sample` command contract. Evidence is `just oracle::run --case smoke/tiny-circuit`, the M8 `sample` milestone tasks, and the CLI compatibility order.

## 2026-06-26 - M0: Benchmark Smoke Before Benchmark Harness

Status: Resolved
Revealed by: milestone audit and full-code-review of `just bench::smoke`.
Current text: M0 requires CI benchmark smoke tests, while M3 owns the benchmark package, baseline measurements, benchmark matrix, and performance contracts.
Gap: before M3, benchmark smoke can only prove workspace wiring unless the plan requires an explicit placeholder benchmark target.
Proposed amendment: clarify whether M0 benchmark smoke is compile-only workspace smoke or require a tiny explicit benchmark target that is intentionally replaced by the M3 benchmark harness.
Resolution: M0 now defines `just bench::smoke` as a compile and wiring smoke for benchmark operations only. It must not claim benchmark baselines, performance thresholds, or workload parity before M3 creates the real benchmark package, baseline commands, and benchmark matrix. Evidence is `just bench::smoke` and the M3 benchmark-baseline milestone text.
