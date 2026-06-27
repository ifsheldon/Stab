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

## 2026-06-28 - M12: Beta Gate Scope For Contract-Only Primary Rows

Status: Open
Revealed by: M12 primary compare evidence after reclassifying the M8 primary sampling rows, `m8-sample-high-repeat-contract`, `m9-m2d-bitpacked-contract`, `m9-detect-primary-matrix-contract`, `m9-m2d-primary-matrix-contract`, `m10-analyze-errors-high-repeat-contract`, and four M11 sample_dem rows from `contract-only` to faithful public `stim-cli` baselines.
Current text: M12 says the frozen primary matrix is every benchmark contract row from M4 through M11 except baseline metadata anchors, and completion-style performance runs should pass `--require-beta-gate`, which fails when any selected row lacks a proven Stab-vs-Stim ratio or exceeds the 2.0x beta performance gate.
Gap: the primary matrix still includes `m4-circuit-canonical-print`, `m7-convert-stim-canonical`, and `m10-dem-print-contract`, whose best current evidence is Stab-only contract timing because pinned Stim v1.16.0 has no matching public CLI or `stim_perf` baseline for the exact workload, so the strict beta gate cannot pass the full primary matrix without either converting those rows to comparable baselines, excluding explicitly contract-only rows from beta-gate selection, or replacing them with faithful comparable benchmark rows.
Proposed amendment: define an M12 beta-gate selection rule that separates comparable primary rows from source-owned contract-representative rows, then require `--require-beta-gate` for every comparable primary row and require each remaining contract-representative row to have either a promoted faithful Stim baseline or an explicit follow-up entry explaining why no ratio can be proven before beta.
Resolution: Pending.

## Resolved Entries

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

## Open Entries

## 2026-06-27 - M9: Feedback-Removal Conversion Scope

Status: Open
Revealed by: implementing `stab m2d` and inspecting pinned Stim `command_m2d.test.cc` plus `transform_without_feedback.test.cc`.
Current text: M9 requires `stim m2d` with measurement input parsing, detector conversion, observable output, and inconsistent-input errors, and the compatibility matrix assigns `transform_without_feedback.test.cc` to M9.
Gap: the milestone does not explicitly say whether `m2d --ran_without_feedback` and circuit feedback inlining are required for the initial M9 CLI surface, even though pinned Stim tests exercise that path and Stab currently rejects the flag instead of silently returning incorrect output.
Proposed amendment: add an explicit M9 task and fixture group for `--ran_without_feedback` if feedback-removal parity is required now, or defer it to a named later detector-conversion submilestone while requiring the CLI to reject the flag with a clear error.
Resolution: pending plan update.

## 2026-06-27 - M9: Detector Analysis Utility Row Ownership

Status: Open
Revealed by: M9 oracle manifest after implementing detector sampling, measurement-to-detection conversion, observable output, and M9 benchmark runners.
Current text: M9 links detector-conversion workflows and the compatibility matrix assigns `circuit_to_detecting_regions.test.cc`, `missing_detectors.test.cc`, and `transform_without_feedback.test.cc` to M9.
Gap: the milestone objective and tasks describe public `detect` and `m2d` workflows, but they do not define public Rust APIs, fixture subsets, or done criteria for detecting regions, missing-detector analysis, or feedback-removal transforms; those rows remain manifest-only while the public CLI/core conversion rows are implemented.
Proposed amendment: split M9 into explicit public workflow acceptance and detector-analysis utility acceptance, naming the APIs and upstream subcases required for each utility row, or move the utility rows to the DEM/analyzer milestone that introduces the required analysis structures.
Resolution: pending plan update.

## 2026-06-27 - M9: Sweep-Conditioned Detection Conversion Scope

Status: Open
Revealed by: milestone audit against pinned Stim `measurements_to_detection_events.test.cc`.
Current text: M9 requires measurement-to-detection conversion from measurement records and circuits with detectors, observables, coordinate shifts, and repeats, but it does not mention sweep data or sweep-conditioned detector expectations.
Gap: upstream measurement-to-detection tests include sweep-bit inputs that can alter expected detector parities through sweep-controlled operations, while the current Stab converter and `m2d` CLI accept only measurements, a circuit, and reference-sample options.
Proposed amendment: state whether sweep-conditioned conversion is in M9 scope. If it is, require typed sweep inputs, `--sweep` and `--sweep_format` CLI flags, and fixtures for sweep count mismatches and sweep-controlled parity changes. If not, move those upstream subcases to the first milestone that introduces sweep-aware simulation.
Resolution: pending plan update.

## 2026-06-27 - M9: Benchmark Baseline Completeness

Status: Open
Revealed by: milestone audit of `just bench::compare --milestone M9` and `just bench::compare --milestone M9 --strict`.
Current text: M9 requires `just bench::compare --milestone M9` to report `detect` and `m2d` throughput separately for text and bit-packed formats, while the benchmark plan describes comparisons against pinned Stim v1.16.0.
Gap: the non-strict compare command reports Stab-side M9 timings, but the current baseline artifact has no M9 pinned Stim rows, so `--strict` fails and the command is not a complete Stab-vs-Stim comparison.
Proposed amendment: either require M9 to record selected pinned Stim detect and m2d baselines before completion and run the strict comparison, or label M9 benchmark evidence as report-only until M12 freezes the primary performance matrix.
Resolution: pending plan update.

## 2026-06-27 - M9: Detection Bit-Packed Format Scope

Status: Resolved
Revealed by: milestone audit against pinned Stim `command_detect.cc`, `command_m2d.cc`, and `measurements_to_detection_events.test.cc`.
Current text: M9 requires `stim detect` with bit-packed modes and `stim m2d` with measurement input parsing, while the benchmark plan names text and bit-packed input.
Gap: the milestone does not say whether M9 bit-packed parity means the `b8` subset needed by current decoder workflows or every Stim v1.16.0 bit-packed format including `ptb64`, nor does it name zero-width bit-packed input behavior as an acceptance case.
Proposed amendment: define the exact M9 bit-packed parity boundary by command and stream: `b8` for public detector and observable streams, `ptb64` for `detect` detector output and `detect --obs_out`, and `ptb64` for `m2d` measurement input only. Require `m2d --out_format=ptb64` and `m2d --obs_out_format=ptb64` to reject like pinned Stim v1.16.0, require zero-width `ptb64` input rejection, and require decoded-record bounds before allocation.
Resolution: M9 now requires `b8` parity for public `detect` and `m2d` detector and observable streams, `ptb64` parity for `detect` detector output, `detect --obs_out`, and `m2d` measurement input, plus explicit rejection for `m2d` `ptb64` detector and observable outputs. Evidence is `cargo test -p stab-cli m9`, including `detect_writes_ptb64_detector_and_observable_outputs`, `detect_rejects_ptb64_shots_that_are_not_multiple_of_64`, `m2d_reads_ptb64_records_and_writes_supported_formats`, `m2d_rejects_ptb64_detector_output_like_stim`, `m2d_rejects_ptb64_observable_output_like_stim`, `m2d_rejects_zero_width_ptb64_input`, and `m2d_rejects_excessive_ptb64_decoded_shots_before_expansion`, plus `cargo test -p stab-core result_formats::tests::ptb64_records_are_measurement_major_over_64_shot_groups`.

## 2026-06-27 - M9: Generated Fixture Round-Trip Coverage

Status: Open
Revealed by: milestone audit comparing the M9 task list to current oracle and benchmark evidence.
Current text: M9 says to add round-trip tests for bit-packed input/output and text input/output across circuit fixtures generated in M7.
Gap: current M9 exact oracle rows use hand-authored circuits and measurement records, while generated repetition-code coverage exists in benchmark runners instead of runnable oracle or test acceptance rows; the plan does not define the generated fixture matrix, output formats, round-trip direction, or whether benchmark primary-matrix representatives count as acceptance evidence.
Proposed amendment: add explicit generated-fixture M9 oracle or Rust tests for selected M7 repetition, rotated surface, unrotated surface, and color-code circuits across `01`, `dets`, and `b8` conversion paths, or narrow the task to say generated-fixture coverage is benchmark evidence only until the primary matrix is frozen.
Resolution: pending plan update.

## 2026-06-27 - M9: Pauli-Target Observable Detection Scope

Status: Resolved
Revealed by: full code review against pinned Stim `frame_simulator` observable handling.
Current text: M9 requires `stim detect` with observables and detector output handling.
Gap: the milestone did not distinguish measurement-record observables from `OBSERVABLE_INCLUDE` Pauli target observables. The prior implementation rejected Pauli-target observables for `detect` to avoid silently returning incorrect logical flips, while `m2d` continued to ignore Pauli targets like pinned Stim's measurement-to-detection converter.
Proposed amendment: either require M9 to implement frame-simulator-style Pauli-target observable flips for `detect`, including deterministic and random observable fixtures, or defer Pauli-target observable detection to the simulator-completeness milestone while requiring an explicit error in the M9 CLI and Rust API.
Resolution: M9 now requires `stim detect` to implement frame-simulator-style Pauli-target observable flips for the documented scalar frame subset while leaving `m2d` conversion behavior unchanged. Evidence is `coverage-simulators-frame-simulator-pauli-observables`, `cargo test -p stab-core detection_sampling`, including RX/RY/RZ Pauli observable parity, product-measurement frame updates, and reference-sample measurement-bit cancellation, plus `cargo test -p stab-cli detect_supports_pauli_target_observable_flips`, `cargo test -p stab-cli detect_supports_product_measurements_with_pauli_observable_flips`, and `cargo test -p stab-cli m2d_ignores_pauli_target_observables_like_stim_conversion`.

## 2026-06-27 - M9: Detection Conversion Streaming And Scale Limits

Status: Open
Revealed by: full code review of `detect` and `m2d` resource behavior.
Current text: M9 requires decoder-pipeline detection workflows and benchmark reporting but does not define streaming, batching, loop-folding, or maximum supported record sizes.
Gap: current Stab materializes measurement records and detection records in memory and unrolls detection-conversion repeats within explicit temporary limits. This prevents unbounded CPU or memory use for hostile inputs, but it is not a final decoder-scale streaming design and does not match Stim's ability to process large files and folded repeats efficiently.
Proposed amendment: add a follow-up milestone or M12 task for compiled/streaming detection conversion that processes records in bounded batches, preserves repeat structure where possible, avoids duplicate sampler analysis, documents or removes temporary limits, and includes benchmark rows for large generated-code detector workloads.
Resolution: pending plan update.

## 2026-06-27 - M8: Skip Loop Folding Scope

Status: Open
Revealed by: milestone audit of M8 `--skip_loop_folding` evidence.
Current text: M8 requires repeat handling, reference sample behavior, and `stim sample` core flags, but does not say whether `--skip_loop_folding` must change the Rust sampler implementation or only be accepted with output-compatible behavior.
Gap: Stab currently accepts `--skip_loop_folding` and proves output parity on a repeat circuit, while optimized loop-folded reference-sample construction remains deferred by the `coverage-util-top-reference-sample-tree` manifest row. The milestone text does not state whether that is sufficient for M8 completion.
Proposed amendment: state that M8 requires `--skip_loop_folding` to be accepted and output-compatible for repeat circuits, while optimized loop-folded reference-sample construction and performance parity are deferred to M12 unless promoted earlier.
Resolution: pending plan update.

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

Status: Open
Revealed by: milestone audit of M6 `circuit_flow_generators`, `has_flow`, `circuit_inverse_qec`, `simplified_circuit`, `mbqc_decomposition`, `circuit_vs_tableau`, and `stabilizers_to_tableau` rows.
Current text: M6 links related util-top tests when their dependencies are in scope, but the oracle manifest records several rows as implemented with notes that defer measurement-rich, detector, noise, sampled-flow, full-gate, tableau-to-circuit, and fuzz variants.
Gap: the milestone does not split deterministic unitary/tableau subset parity from full upstream util-top parity, so an implemented row can be misread as full Stim parity for the entire upstream file.
Proposed amendment: split each related util-top row into explicit subcases owned by M6 and deferred subcases owned by the simulator, detector, or performance-hardening milestones; require public APIs for subset helpers to document unsupported semantics until the deferred rows are implemented.
Resolution: pending plan update.

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

Status: Open
Revealed by: milestone audit of `just bench::compare --milestone M6`.
Current text: M6 requires `just bench::compare --milestone M6` to report Pauli, Clifford, tableau, tableau-iterator, and stabilizers-to-tableau workloads, while benchmark manifest rows point at upstream random, fuzz-like, and large-tableau perf filters.
Gap: the milestone does not distinguish report-only deterministic Stab benchmark runners from exact parity with upstream random and 10K-qubit perf workloads.
Proposed amendment: require M6 compare output to provide deterministic Stab-side timings and normalized rates for each M6 benchmark row, label non-exact benchmark workloads in compare notes, and defer exact random and large-tableau threshold parity to M12 performance hardening after random hooks and optimized tableau internals are specified.
Resolution: pending plan update.

## 2026-06-27 - M6: Stabilizer Algebra Public View And Text Scope

Status: Resolved
Revealed by: implementation of the first owned Pauli-string algebra slice and upstream stabilizer scan.
Current text: M6 requires `PauliString`, `CliffordString`, `Tableau`, related iterators or views, sign handling, and text round trips.
Gap: the milestone does not say whether Rust must expose a public borrowed `PauliStringRef` equivalent, does not distinguish real-phase C++ `PauliString` text from phase-general `FlexPauliString` sparse and lowercase text, and does not define which Python-facing phase semantics are required before the Python API milestone.
Proposed amendment: state that M6 starts with owned Pauli, FlexPauli, Clifford, and Tableau APIs; borrowed views may stay internal unless a later M6 task proves a public view is necessary; text parity must separately cover real dense `PauliString` syntax and phase-general `FlexPauliString` dense or sparse syntax; Python-only binding behavior is semantic-mining input but not a public API requirement until the Python milestone.
Resolution: M6 now states that the public Rust API starts with owned `PauliString`, `FlexPauliString`, `CliffordString`, and `Tableau` values, public `PauliStringRef` parity is not required unless later parity or performance work proves it necessary, and Python-only binding behavior remains semantic-mining input until the Python API milestone. `crates/stab-core/tests/stabilizers.rs` now checks that real `PauliString` rejects imaginary, lowercase, and sparse-style text while `FlexPauliString` accepts phase-general dense and sparse text with canonical display; manifest rows `coverage-stabilizers-pauli-string`, `coverage-stabilizers-flex-pauli-string`, and `coverage-stabilizers-pauli-string-ref` track the split.

## 2026-06-27 - M4: Gate Decomposition Utility Scope

Status: Open
Revealed by: implementation of `coverage-circuit-gate-decomposition` as a direct Rust oracle row.
Current text: M4 links `src/stim/circuit/gate_decomposition.test.cc` under Circuit Model, Parser, Targets, And Decomposition, but M4's objective is the public `.stim` data model, gate metadata, parser, validator, and canonical printer.
Gap: the upstream file mixes pure circuit-structure helpers, such as target grouping and disjoint segmentation, with semantic MPP/SPP decomposition behavior that later depends on base-gate decomposition, flows, tableaus, and simulator correctness.
Proposed amendment: state that M4 owns structural decomposition prerequisites only, including Pauli-product grouping and disjoint target segmentation; full `decomposed` behavior for MPP, SPP, pair measurements, and base-gate lowering should move to the first milestone that implements the required tableau/simulator semantics or receive its own explicit milestone task.
Resolution: pending plan update.

## 2026-06-27 - M4: Probability Utility Fixture Scope

Status: Open
Revealed by: implementation of `coverage-util-bot-probability-util` as a direct Rust oracle row.
Current text: M4 requires gate argument rules and probability validation, while the test-porting plan points at `src/stim/util_bot/probability_util.test.cc` for probability validation.
Gap: the referenced upstream file also tests `sample_hit_indices` and biased random bit generation, which require RNG and bit-storage behavior that M4 does not otherwise define.
Proposed amendment: state that M4 owns only closed-unit probability validation and disjoint probability-list validation from this file; random hit-index sampling and biased bit generation should move to the first milestone that introduces equivalent RNG and bit/sampler APIs.
Resolution: pending plan update.

## 2026-06-27 - M2: Manifest-Only Subcase Granularity

Status: Open
Revealed by: milestone audit of the M2 manifest coverage rows.
Current text: M2 and the test-porting plan allow red or manifest-only oracle cases for all P0 and P1 files needed by M4 through M11.
Gap: file-level manifest-only rows can satisfy coverage without identifying the upstream subcases, fixture families, malformed-input cases, or extraction criteria that future implementation milestones must port.
Proposed amendment: require manifest-only rows to name planned subcase groups or extraction criteria for each upstream test file before the owning implementation milestone starts.
Resolution: pending plan update.

## 2026-06-26 - M0: Upstream Smoke References Overreach

Status: Open
Revealed by: milestone audit of the M0 oracle lab implementation.
Current text: M0 links `src/stim.test.cc`, `src/stim/main_namespaced.test.cc`, and `src/stim_included_twice.test.cc` as C++ smoke references.
Gap: those upstream files include behavior from later milestones, including circuit parsing, gate metadata, analyzer behavior, and richer CLI mode handling, so treating the full files as M0 requirements would pull M4, M6, and M10 work into the foundation milestone.
Proposed amendment: clarify that M0 extracts only oracle-process smoke checks from these files, specifically help-command health, main binary namespacing health, and one tiny deterministic circuit case; all parser, gate table, analyzer, and broader CLI behavior stays with later milestones.
Resolution: pending plan update.

## 2026-06-26 - M0: Oracle Tiny Sample Shim Boundary

Status: Open
Revealed by: milestone audit and full-code-review of the M0 `stab-cli sample` smoke shim.
Current text: M0 requires `just oracle::run --case smoke/tiny-circuit`, while the CLI compatibility order defers real `sample` support to M8.
Gap: the plan does not say whether a minimal M0 sample command counts as CLI compatibility or is only an oracle fixture target.
Proposed amendment: state that any M0 sample path is an oracle-only smoke shim and does not count as implemented `stim sample` compatibility; M8 remains responsible for the public `sample` command contract.
Resolution: pending plan update.

## 2026-06-26 - M0: Benchmark Smoke Before Benchmark Harness

Status: Open
Revealed by: milestone audit and full-code-review of `just bench::smoke`.
Current text: M0 requires CI benchmark smoke tests, while M3 owns the benchmark package, baseline measurements, benchmark matrix, and performance contracts.
Gap: before M3, benchmark smoke can only prove workspace wiring unless the plan requires an explicit placeholder benchmark target.
Proposed amendment: clarify whether M0 benchmark smoke is compile-only workspace smoke or require a tiny explicit benchmark target that is intentionally replaced by the M3 benchmark harness.
Resolution: pending plan update.
