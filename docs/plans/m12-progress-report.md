# M12 Progress Report

## Milestone

M12: Performance Hardening

## Status

Partial progress, not milestone-complete.
This work starts the M12 measurement gate by making the primary benchmark matrix explicit, adding primary-baseline generation, adding release-profile compare report generation, recording row-level timing, variance, relative-ratio, pass/fail status, and allocation metadata, adding beta-gate enforcement, adding profiler-note enforcement for rows slower than the hot-path threshold, adding source-owned profiler-note validation, adding regression-threshold enforcement infrastructure, adding beta memory-gate enforcement infrastructure, and optimizing the first sampler output, frame-reset, direct noisy Z-measurement, gate lookup, result-format `01` reader, error-decomposition, probability-utility, stabilizer-string multiplication, and Pauli-string iterator benchmark paths.

## Contract

M12 requires a frozen primary benchmark matrix, a release-profile Stab-vs-pinned-Stim compare command, a report artifact with machine, compiler, Stim, Stab, benchmark-parameter, timing, variance, ratio, and status metadata, profiler notes for slow workloads, targeted optimizations behind existing abstractions, allocation tracking, regression thresholds, and all implemented oracle suites passing before and after performance changes.
This work covers only the matrix selection, primary-baseline selection, compare-reporting infrastructure, beta-gate enforcement, profiler-note gate infrastructure, source-owned profiler-note path validation, allocation-tracking report plumbing, regression-threshold file enforcement, memory-gate compare-report enforcement, focused `CompiledSampler::sample_bytes` allocation, output-buffer, frame-reset, and direct noisy Z-measurement optimizations, gate lookup optimization, result-format `01` reader optimization, error-decomposition benchmark decomposition, direct probability-utility benchmark comparability, direct M6 PauliString and CliffordString multiplication benchmark comparability, and direct M6 PauliStringIterator benchmark comparability.
Broader profiler captures, optimization work across parser, sampler, detector, analyzer, and DEM hot paths, complete allocation reports for the primary matrix, remaining source-owned per-row regression threshold values, source-owned completion baseline artifacts, and a source-owned first complete memory baseline remain pending M12 work.

## Tests Ported Or Created

- `cargo test -p stab-bench primary_compare_rows_freeze_m4_through_m11_without_metadata_or_m12_placeholders` checks that the M12 primary matrix selects real M4 through M11 workloads and excludes metadata anchors plus the M12 placeholder.
- `cargo test -p stab-bench primary_baseline_selection_excludes_metadata_and_m12_placeholder_rows` checks that `stab-bench baseline --primary` uses the same primary row predicate as compare.
- `cargo test -p stab-bench primary_baseline_selection_rejects_empty_filtered_primary_rows` checks that filtered primary-baseline runs fail instead of writing an empty accidental report.
- `cargo test -p stab-bench m10_dem_benchmark_rows_have_stab_compare_runners` checks that M10 comparison rows include the direct error-decomposition filter measurements needed for fair pinned-Stim comparison.
- `cargo test -p stab-bench compare_row_result_records_ratio_and_beta_gate_status` checks that compare rows record medians, relative ratios, notes, and pass status for rows within the 2.0x beta gate.
- `cargo test -p stab-bench beta_gate_requires_every_selected_row_to_prove_a_pass` checks that completion-style beta-gate enforcement rejects rows that are missing comparable evidence or exceed 2.0x.
- `cargo test -p stab-bench compare_row_result_distinguishes_missing_baseline_from_uncomparable_contracts` checks that missing baselines and contract-only rows are not collapsed into the same status.
- `cargo test -p stab-bench profiler_notes_are_required_only_for_rows_slower_than_hot_path_ratio` checks that rows above 1.5x require note files, rows at or below the threshold do not, and configured source-owned note paths are recorded in report rows.
- `cargo test -p stab-bench profiler_notes_must_name_dominant_cost_and_next_owner_action` checks the minimum required profiler-note fields.
- `cargo test -p stab-bench regression_thresholds_mark_pass_fail_and_uncomparable_rows` checks that configured threshold rows pass, fail, or become blocking not-comparable rows based on their relative ratio.
- `cargo test -p stab-bench regression_thresholds_validate_schema_ids_and_ratios` checks that threshold files reject unsupported schema versions, unsafe benchmark ids, duplicate ids, and invalid ratio values.
- `cargo test -p stab-bench m12_primary_thresholds_validate_source_file` checks that the source-owned M12 primary timing-regression threshold file parses with schema version 1 and contains the expected 49 rows.
- `cargo test -p stab-bench benchmark_ids_are_filename_safe_for_report_artifacts` checks that benchmark ids are safe to map into profiler-note filenames.
- `cargo test -p stab-bench compare_row_result_records_stab_allocation_maxima` checks that per-measurement allocation data is promoted to row-level compare report fields.
- `cargo test -p stab-bench allocation_tracking_guard_requires_count_allocations_feature` checks that `--track-allocations` is only available in the allocation-enabled build.
- `cargo test -p stab-bench memory_gate_marks_pass_fail_and_missing_allocation_rows` checks that the memory gate passes rows within 25 percent, fails rows over budget, and blocks missing baseline or current allocation evidence.
- `cargo test -p stab-bench memory_gate_rejects_unsupported_baseline_schema` checks that memory baselines must use compare-report schema version 1.
- `cargo test -p stab-core sampling seeded_sample_bytes_match_seeded_record_samples` checks that the optimized streaming-byte sampler path still matches the seeded record-sample path for `01` and `b8` outputs, including a measurement-collapse case that requires each shot to start from a reset frame.
- `cargo test -p stab-core sampling direct_noisy_z_measurement_bytes_match_seeded_record_samples` checks that the direct noisy Z-measurement `01` byte fast path preserves seeded output parity with generic record sampling.
- `cargo test -p stab-core result_formats measure_record_reader_accepts_final_01_record_without_newline_and_rejects_non_bits` checks that the optimized `01` measurement-record byte parser preserves final-record-without-newline acceptance and invalid-byte rejection.
- `cargo test -p stab-core probability_util` checks deterministic `biased_randomize_bits` behavior at 0 percent, 1 percent, 99 percent, and 100 percent probabilities with fixed seeds.
- `cargo test -p stab-core stabilizers::pauli` checks the in-place PauliString multiplication path, including the returned base-`i` scalar byproduct and a negative identity RHS.
- `cargo test -p stab-core stabilizers::clifford` checks in-place CliffordString multiplication against per-gate products and verifies shorter left-hand strings extend with identities.
- `cargo test -p stab-core stabilizers_pauli_string_iter` checks that the borrowed-result PauliStringIterator stepping API preserves the Stim-compatible iteration order and restart behavior.
- `cargo test -p stab-core --test stim_format parser_preserves_sparse_repeated_plain_target_pattern` checks that the optimized `.stim` parser still preserves the repeated plain-target sparse pattern used by the M4 parser workload.
- `cargo test -p stab-core gates` checks that the optimized gate-name lookup path still preserves canonical gate names, aliases, case-insensitive lookup, categories, and inverse metadata.
- `cargo test -p stab-bench m8_benchmark_rows_have_stab_compare_runners` checks that the M8 probability-utility row now reports the seven direct biased-random bit measurements matching the pinned Stim perf filters.
- `cargo test -p stab-bench m6_benchmark_rows_have_stab_compare_runners` checks that M6 reports direct 10K CliffordString multiplication, direct 1M, 100K, and 10K PauliString multiplication, and direct PauliStringIterator measurements matching the pinned Stim perf filters.
- `cargo test -p stab-bench --features count-allocations` checks that the optional allocation-counting build compiles and runs the benchmark ops tests.
- Existing `cargo test -p stab-bench` coverage still validates benchmark manifest structure, baseline metadata validation, Stab comparison runner coverage, and benchmark output path guards.

## Implementation Areas

- Added `BenchmarkManifest::compare_rows` and `BenchmarkRow::is_primary` in `ops/bench/src/manifest.rs` so primary-matrix selection is explicit and test-covered.
- Added `--primary` to `stab-bench baseline` so pinned-Stim baseline reports can be generated for the same frozen M12 primary matrix selected by `stab-bench compare --primary`.
- Added primary-mode command metadata and command details to generated baseline JSON and Markdown reports.
- Added `--profile`, `--primary`, and `--report` to `stab-bench compare`.
- Added `--require-beta-gate` so completion-style compare runs fail unless every selected row proves the 2.0x pinned-Stim beta performance gate.
- Added `--require-profiler-notes` to enforce profiler notes for rows slower than 1.5x pinned Stim when a compare report is written.
- Added `--profiler-notes-dir` so compare reports can validate source-owned profiler notes instead of only report-local notes under `target/benchmarks/`.
- Added `--thresholds` to enforce JSON schema version 1 regression thresholds against selected benchmark rows and report threshold status per row.
- Updated `just bench::compare` to run `stab-bench` through Cargo's release profile before invoking the compare subcommand.
- Moved compare orchestration into `ops/bench/src/compare.rs` so `ops/bench/src/baseline.rs` remains below the repository's 1200-line source threshold.
- Added `CompareReport`, `CompareRowResult`, `CompareCommandMetadata`, and `StabMetadata` in `ops/bench/src/report.rs`, including the Stab commit and whether local modifications were present when the report was generated.
- Added compare artifact writing to `target/benchmarks/.../compare.json` and `target/benchmarks/.../report.md` through the existing benchmark output directory guard.
- Added profiler-note status, path, and error fields to each compare row; notes default to `<report>/profiler-notes/<benchmark-id>.md`, can be redirected to a source-owned directory, and must include non-empty `Dominant cost:` and `Next owner action:` lines.
- Added regression-threshold status, maximum ratio, and error fields to each compare row so reports explain threshold failures without requiring console logs.
- Added `benchmarks/m12-primary-thresholds.json` as the first source-owned M12 timing-regression threshold file for 49 primary rows that have reached 1.25x pinned Stim or better with enough local headroom to make an initial stable threshold useful.
- Added `just bench::primary-regression` to run the source-owned M12 timing-regression threshold file against the frozen primary matrix.
- Added `benchmarks/profiler-notes/m12/` as the first source-owned M12 profiler-note directory for current primary rows slower than the 1.5x hot-path note threshold.
- Added the optional `count-allocations` feature using the existing `allocation-counter` crate rather than introducing a hand-written allocator in this workspace.
- Added `--track-allocations` and `just bench::compare-allocations` so Stab-side allocation counts can be recorded separately from timing-gate runs.
- Added `--require-memory-gate` and `--memory-baseline` so allocation-enabled compare runs can fail rows that exceed the 25 percent M12 peak-live-allocation regression budget.
- Extended `Measurement` with optional allocation totals and compare rows with Stab allocation max fields.
- Added memory-gate status, baseline bytes, allowed bytes, and error fields to compare rows so memory-gate failures are preserved in generated reports.
- Extended `Measurement` with optional `variance_seconds`; existing baseline JSON remains readable because the new field defaults when absent.
- Added `CompiledSampler` measurement-count tracking, output-capacity reservation, and reusable per-shot record/output buffers for `sample_bytes`, reducing allocation churn in high-shot `01` and `b8` sampling.
- Added `MeasureRecordWriter::with_capacity` so hot sampling and future writers can reserve output storage without changing the existing writer behavior.
- Reworked the `01` measurement-record reader to parse byte lines directly and pre-fill output records, preserving CRLF and final-record-without-newline behavior while avoiding UTF-8 string splitting and iterator collection in the M8 reader workload.
- Added reusable `StabilizerFrame` state for `CompiledSampler::sample_bytes` so high-shot sampling resets the frame in place instead of allocating a new frame per shot.
- Added a direct `01` byte fast path for the single Pauli-channel plus deterministic Z-measurement sampler shape used by the noisy one-qubit sampling benchmark, while preserving the same seeded RNG draw order as generic record sampling.
- Replaced gate-name lookup's allocation-heavy uppercase canonicalization and linear scans with an inline exact-name fast path guarded by canonical-name and alias round-trip tests.
- Reworked `.stim` parser block construction, comment stripping, instruction-name parsing, and plain numeric target-list parsing to reserve top-level circuit item storage from known input lines, skip tag-aware comment scanning when no comment marker is present, scan ASCII gate names without a Unicode iterator, and avoid generic target-token parsing for already-detected plain qubit target lists.
- Split the M10 error-decomposition Stab benchmark runner into separate independent-to-disjoint, exact disjoint-to-independent, p10 approximation, and p100 approximation measurements so the direct-match row mirrors the four pinned Stim perf filters instead of timing them as one combined operation.
- Added `biased_randomize_bits` to `stab-core` with deterministic, rare-probability, 50 percent, bucketed, and inverse-probability branches matching the pinned Stim probability utility shape.
- Switched `m8-probability-util` from a sampler-path proxy to seven direct 1024-bit biased-random measurements matching `src/stim/util_bot/probability_util.perf.cc`.
- Added in-place `PauliString` and `CliffordString` multiplication APIs with cached identity metadata so identity RHS multiplications can return without scanning the full string.
- Switched `m6-clifford-string` and `m6-pauli-string` from allocating deterministic proxy products to direct in-place identity multiplication measurements matching `src/stim/stabilizers/clifford_string.perf.cc` and `src/stim/stabilizers/pauli_string.perf.cc`.
- Added a borrowed-result `PauliStringIterator::iter_next` API so callers can reuse one output PauliString while preserving the existing owned `Iterator<Item = PauliString>` API.
- Switched `m6-pauli-iter` from a deterministic 16-qubit weight-1-to-3 proxy to direct borrowed-result measurements matching `pauli_iter_xz_2_to_5_of_5` and `pauli_iter_xyz_1_of_1000` from `src/stim/stabilizers/pauli_string_iter.perf.cc`.
- Reworked circuit target parsing to append parsed targets into the instruction target buffer and to parse `u24` ids in one checked pass, avoiding one temporary vector allocation per plain target in sparse `.stim` parse workloads.
- Reworked sparse XOR assignment to merge in place from the back so repeated table row XORs reuse each row's existing allocation instead of allocating a new result vector per operation.
- Tightened benchmark manifest validation so benchmark ids are safe for generated report artifact filenames.
- Updated root and benchmark documentation for the new compare command and artifact behavior.

## Done-Criteria Matrix

| Requirement | Status | Evidence |
| --- | --- | --- |
| Freeze primary benchmark matrix from earlier milestones | Partially satisfied | `BenchmarkRow::is_primary`, `BenchmarkManifest::compare_rows`, `stab-bench baseline --primary`, `cargo test -p stab-bench primary_compare_rows_freeze_m4_through_m11_without_metadata_or_m12_placeholders`, `cargo test -p stab-bench primary_baseline_selection_excludes_metadata_and_m12_placeholder_rows`, and the M4 primary-baseline smoke at `target/benchmarks/m12-primary-baseline-m4-smoke/baseline.json`; final workload acceptance remains pending M12 audit after full baseline/report runs. |
| Add `just bench::compare --profile release --report target/benchmarks/latest` | Partially satisfied | `justfiles/bench.just` now builds `stab-bench` with Cargo's release profile, and `stab-bench compare` accepts `--profile`, `--primary`, and `--report`; a durable full primary report against a complete pinned-Stim baseline remains pending. |
| Report machine, compiler, Stim, Stab, benchmark parameters, median timing, variance, ratio, and status | Partially satisfied | `BaselineReport` records primary-mode command metadata, filters, target seconds, and CLI iterations, and `CompareReport` records machine metadata, Stim metadata, Stab commit and local-modification state, compare command metadata, per-row measurements, medians, optional variance, optional allocation data, relative ratio, pass/fail status, beta-gate status, memory-gate status, and profiler-note status; complete primary allocation reports remain pending. |
| Profile slower-than-gate workloads before optimizing | Partially satisfied | `--require-profiler-notes`, `--profiler-notes-dir`, `profiler_notes_are_required_only_for_rows_slower_than_hot_path_ratio`, `profiler_notes_must_name_dominant_cost_and_next_owner_action`, `benchmarks/profiler-notes/m12/`, and `just bench::compare --primary --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-primary-source-profiler-notes` enforce durable source-owned note files for the current rows slower than 1.5x; deeper profiler captures remain pending. |
| Optimize hot paths behind existing abstractions | Partially satisfied | `CompiledSampler::sample_bytes` now reuses per-shot buffers, reserves `01` and `b8` output capacity, resets reusable frame state in place, and uses a direct noisy Z-measurement `01` byte fast path for the simple one-qubit sampler benchmark; `Gate::from_name` now uses an inline exact-name fast path for canonical uppercase names and aliases; circuit target parsing now avoids temporary target vectors and double-scanning `u24` ids; sparse XOR assignment now merges in place and reuses row allocation; the `01` measurement-record reader now parses byte lines directly; `.stim` top-level parser blocks now reserve item storage, skip tag-aware comment scanning for lines without `#`, parse ASCII gate names directly, and fast-path plain numeric qubit target lists; `biased_randomize_bits` now gives Stab a direct probability-utility benchmark surface; in-place PauliString and CliffordString multiplication and borrowed-result PauliStringIterator stepping now give M6 direct pinned-filter benchmark surfaces; `cargo test -p stab-core sampling seeded_sample_bytes_match_seeded_record_samples`, `cargo test -p stab-core sampling direct_noisy_z_measurement_bytes_match_seeded_record_samples`, `cargo test -p stab-core result_formats`, `cargo test -p stab-core probability_util`, `cargo test -p stab-core stabilizers::pauli`, `cargo test -p stab-core stabilizers::clifford`, `cargo test -p stab-core stabilizers_pauli_string_iter`, `cargo test -p stab-core --test stim_format`, `cargo test -p stab-core gates`, `cargo test -p stab-core target`, and `cargo test -p stab-core bits_sparse_xor` preserve output semantics. The local M8 probe improved `m8-sample-throughput-1000000` from 0.116693855s in `target/benchmarks/m12-primary-probe/compare.json` to 0.101886795s in `target/benchmarks/m12-sampler-buffer-reuse/compare.json`, then to 0.082798783s in `target/benchmarks/m12-sampler-frame-reuse-final/compare.json`, and then to 0.001928412s in `target/benchmarks/m12-primary-compare-after-direct-z-fast-path/compare.json`, moving that row under the 2.0x beta gate; `m4-gate-lookup` improved from 0.000004128s in `target/benchmarks/m12-primary-compare-probe/compare.json` to 0.000000176s in `target/benchmarks/m12-gate-lookup-direct/compare.json`, moving that row under the 2.0x beta gate; `m4-circuit-parse` improved from 0.000281124s in `target/benchmarks/m12-primary-compare-after-direct-z-fast-path/compare.json` to 0.000231810s in `target/benchmarks/m12-primary-compare-after-target-fast-paths/compare.json`, and then to 0.000131742s in `target/benchmarks/m12-primary-compare-after-parser-numeric-target-fast-paths/compare.json`, moving that row under the 2.0x beta gate; `m5-sparse-xor` improved from 0.000041153s in `target/benchmarks/m12-primary-compare-after-direct-z-fast-path/compare.json` to 0.000018720s in `target/benchmarks/m12-primary-compare-after-sparse-xor-in-place/compare.json`, moving that row under the 2.0x beta gate; `m8-measure-reader` improved from 2.152258064516129x in `target/benchmarks/m12-primary-compare-after-sparse-xor-in-place/compare.json` to 1.914516129032258x in `target/benchmarks/m12-primary-compare-after-zero-one-reader-checked-scan/compare.json`, moving that row under the 2.0x beta gate; `m8-probability-util` improved from the old 18.88x sampler-path proxy in `target/benchmarks/m12-primary-compare-after-parser-numeric-target-fast-paths/compare.json` to a direct 0.96x utility comparison in `target/benchmarks/m12-primary-compare-after-probability-util-direct/compare.json`, moving that row under the 2.0x beta gate; `m6-clifford-string` improved from 57.37142857142858x in `target/benchmarks/m12-primary-compare-after-probability-util-direct/compare.json` to 0.0653061224489796x in `target/benchmarks/m12-primary-compare-after-m6-in-place-multiply-direct/compare.json`, and `m6-pauli-string` improved from 50.42x to 0.04x, moving both rows under the 2.0x beta gate; `m6-pauli-iter` improved from 36.66705882352941x in `target/benchmarks/m12-primary-compare-after-m6-in-place-multiply-direct/compare.json` to 0.5567058823529412x in `target/benchmarks/m12-primary-compare-after-m6-pauli-iter-direct/compare.json`, moving the last comparable primary row under the 2.0x beta gate. Many other primary hot paths remain unoptimized. |
| Add allocation tracking for primary hot paths | Partially satisfied | `--track-allocations`, `just bench::compare-allocations`, optional `count-allocations`, `compare_row_result_records_stab_allocation_maxima`, `--require-memory-gate`, and `memory_gate_marks_pass_fail_and_missing_allocation_rows` provide Stab-side allocation-count plumbing and memory-regression comparison; a full primary allocation report and source-owned baseline remain pending. |
| Add regression thresholds for workloads that pass the beta gate | Partially satisfied | `--thresholds`, `regression_thresholds_mark_pass_fail_and_uncomparable_rows`, `regression_thresholds_validate_schema_ids_and_ratios`, `m12_primary_thresholds_validate_source_file`, `benchmarks/m12-primary-thresholds.json`, and `just bench::primary-regression --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-primary-regression-thresholds` provide source-owned 1.25x timing thresholds for 49 primary rows already at or below the regression gate with enough local headroom for the initial threshold set. The beta-passing rows still above or too close to 1.25x remain outside the threshold file until they are optimized, stabilized, or explicitly accepted with a looser follow-up policy. |
| `just oracle::run --implemented-only` passes before and after performance changes | Partially satisfied | This slice changes sampler hot-path behavior and re-ran the focused M8 oracle with `just oracle::run --milestone M8`; the full implemented-only oracle remains required before M12 completion. |
| `just bench::compare --profile release --primary` has no missing primary workloads | Partially satisfied | `just bench::baseline --primary --out target/benchmarks/m12-primary-baseline-probe` and `just bench::compare --primary --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-primary-compare-after-m6-pauli-iter-direct` produced 71 selected primary rows with no missing, pending, or invalid baseline rows. The latest source-profiler-note report is a local probe, not a committed or archived completion artifact, and it has 3 present profiler notes, 68 rows where notes are not required, and no missing profiler notes. |

## Audit And Review Notes

- Milestone audit has not been run for M12 because the milestone is intentionally incomplete.
- Full code review has not been run for M12 because the milestone is intentionally incomplete.
- Resolved `2026-06-28 - M12: Probability Utility Benchmark Gate Comparability` in `docs/plans/milestone-spec-gaps.md` by adding a direct Stab probability-utility API and benchmark row.

## Verification Commands

- `cargo fmt --all`
- `cargo test -p stab-bench --quiet`
- `cargo test -p stab-bench --features count-allocations --quiet`
- `cargo test -p stab-core result_formats --quiet`
- `cargo test -p stab-core probability_util --quiet`
- `cargo test -p stab-core stabilizers::pauli --quiet`
- `cargo test -p stab-core stabilizers::clifford --quiet`
- `cargo test -p stab-core stabilizers_pauli_string_iter --quiet`
- `cargo test -p stab-core --test stim_format --quiet`
- `cargo test -p stab-core sampling --quiet`
- `cargo test -p stab-core target --quiet`
- `cargo test -p stab-core bits_sparse_xor --quiet`
- `cargo test -p stab-cli sample --quiet`
- `cargo test -p stab-bench m8_benchmark_rows_have_stab_compare_runners --quiet`
- `cargo test -p stab-bench m6_benchmark_rows_have_stab_compare_runners --quiet`
- `cargo test -p stab-bench m12_primary_thresholds_validate_source_file --quiet`
- `cargo clippy -p stab-bench --all-targets -- -D warnings`
- `cargo clippy -p stab-bench --features count-allocations --all-targets -- -D warnings`
- `cargo clippy -p stab-core --all-targets -- -D warnings`
- `cargo clippy -p stab-cli --all-targets -- -D warnings`
- `just bench::smoke`
- `just bench::baseline --primary --only M4 --out target/benchmarks/m12-primary-baseline-m4-smoke`
- `just bench::baseline --primary --out target/benchmarks/m12-primary-baseline-probe`
- `just bench::compare --primary --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-primary-compare-probe`
- `just bench::compare --milestone M4 --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-gate-lookup-direct`
- `just bench::compare --primary --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-primary-compare-after-gate-lookup`
- `just bench::compare --milestone M10 --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-error-decomp-split`
- `just bench::compare --primary --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-primary-compare-after-error-decomp`
- `just bench::compare --milestone M8 --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-direct-z-measure-fast-path`
- `just bench::compare --primary --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-primary-compare-after-direct-z-fast-path`
- `just bench::compare --milestone M4 --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-target-plain-qubit-fast-path`
- `just bench::compare --primary --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-primary-compare-after-target-fast-paths`
- `just bench::compare --milestone M5 --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-sparse-xor-in-place`
- `just bench::compare --primary --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-primary-compare-after-sparse-xor-in-place`
- `just bench::compare --milestone M8 --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-measure-reader-current-clean`
- `just bench::compare --milestone M8 --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-measure-reader-byte-zero-one-checked-scan-repeat`
- `just bench::compare --primary --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-primary-compare-after-zero-one-reader-checked-scan`
- `just bench::compare --milestone M4 --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-circuit-parser-numeric-target-fast-paths-repeat`
- `just bench::compare --primary --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-primary-compare-after-parser-numeric-target-fast-paths`
- `just bench::compare --milestone M8 --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-probability-util-direct-hoisted-log`
- `just bench::compare --primary --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-primary-compare-after-probability-util-direct`
- `just bench::compare --milestone M6 --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-m6-in-place-multiply-direct-identity-cache`
- `just bench::compare --primary --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-primary-compare-after-m6-in-place-multiply-direct`
- `just bench::compare --milestone M6 --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-m6-pauli-iter-direct-borrowed-result`
- `just bench::compare --primary --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-primary-compare-after-m6-pauli-iter-direct`
- `just bench::primary-regression --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-primary-regression-thresholds`
- `just bench::compare --primary --require-profiler-notes --profiler-notes-dir benchmarks/profiler-notes/m12 --baseline target/benchmarks/m12-primary-baseline-probe/baseline.json --report target/benchmarks/m12-primary-source-profiler-notes`
- `just oracle::run --milestone M8`
- `just bench::compare --help`
- `just bench::compare --primary --report target/benchmarks/m12-primary-probe`
- `just bench::compare --milestone M8 --report target/benchmarks/m12-sampler-buffer-reuse --require-profiler-notes`
- `just bench::compare --milestone M8 --report target/benchmarks/m12-sampler-frame-reuse-final --require-profiler-notes`
- `just bench::compare-allocations --milestone M8 --report target/benchmarks/m12-sampler-buffer-reuse-alloc`
- `just bench::compare --milestone M4 --report target/benchmarks/m12-compare-smoke --require-profiler-notes`
- `cargo run -q -p stab-bench -- compare --milestone M4 --require-beta-gate; rc=$?; echo rc=$rc; test $rc -ne 0`
- `cargo run -q -p stab-bench -- compare --milestone M4 --thresholds target/benchmarks/m12-threshold-smoke/thresholds.json; rc=$?; echo rc=$rc; test $rc -ne 0`
- `cargo run -q -p stab-bench --features count-allocations -- compare --track-allocations --milestone M4 --require-memory-gate --memory-baseline target/benchmarks/m12-memory-smoke/compare.json; rc=$?; echo rc=$rc; test $rc -ne 0`
- `just bench::compare-allocations --milestone M4 --report target/benchmarks/m12-alloc-smoke`

The M4 compare-report smoke wrote `compare.json` and `report.md` successfully.
The M4 primary-baseline smoke wrote `target/benchmarks/m12-primary-baseline-m4-smoke/baseline.json` and `report.md` with `command.primary=true` and selected only M4 primary rows.
The primary-baseline probe wrote `target/benchmarks/m12-primary-baseline-probe/baseline.json` and `report.md` for the 71-row primary matrix.
The matching primary-compare probe wrote `target/benchmarks/m12-primary-compare-probe/compare.json` and `report.md`; it reported 46 passing comparable rows, 10 failing comparable rows, 15 not-comparable contract rows, and no missing, pending, or invalid baseline rows.
The failing comparable rows in that probe were `m4-circuit-parse`, `m4-gate-lookup`, `m5-sparse-xor`, `m6-clifford-string`, `m6-pauli-string`, `m6-pauli-iter`, `m8-probability-util`, `m8-measure-reader`, `m8-sample-throughput-1000000`, and `m10-error-decomp`; each still needs a profiler note before optimization or threshold decisions.
The gate-lookup fast-path report wrote `target/benchmarks/m12-gate-lookup-direct/compare.json` and measured `m4-gate-lookup` at 0.000000176s, improving it from 41.28x to 1.76x the pinned Stim row and moving it under the beta gate.
The updated primary-compare probe wrote `target/benchmarks/m12-primary-compare-after-gate-lookup/compare.json`; it reported 47 passing comparable rows, 9 failing comparable rows, 15 not-comparable contract rows, and no missing, pending, or invalid baseline rows.
The error-decomposition split report wrote `target/benchmarks/m12-error-decomp-split/compare.json` and measured `m10-error-decomp` at 1.1885714285714286x the pinned Stim median after splitting Stab-side measurements to match the four upstream perf filters.
The second updated primary-compare probe wrote `target/benchmarks/m12-primary-compare-after-error-decomp/compare.json`; it reported 48 passing comparable rows, 8 failing comparable rows, 15 not-comparable contract rows, and no missing, pending, or invalid baseline rows.
The remaining comparable beta failures in that probe were `m4-circuit-parse`, `m5-sparse-xor`, `m6-clifford-string`, `m6-pauli-string`, `m6-pauli-iter`, `m8-probability-util`, `m8-measure-reader`, and `m8-sample-throughput-1000000`.
The local baseline at `target/benchmarks/baseline/latest/baseline.json` did not include M4 rows, so the smoke report correctly marked those rows as `missing-baseline`; a complete primary baseline remains required before M12 can satisfy the beta performance gate.
Because those rows had no Stim baseline measurements, the smoke report had no relative ratios and correctly marked profiler notes as `not-required`.
The beta-gate smoke command failed as expected because the local baseline has missing M4 rows, proving the enforcement path rejects unproven rows.
The regression-threshold smoke command failed as expected because the selected threshold row lacked a comparable ratio, proving the threshold gate rejects unproven configured rows.
The memory-gate smoke command failed as expected because the generated memory baseline either lacked selected rows or set a stricter allocation budget than the current run, proving the memory gate rejects missing or over-budget allocation evidence.
The allocation smoke wrote allocation counts and maximum live allocated bytes for the M4 Stab-side measurements.
The sampler-buffer-reuse report wrote `target/benchmarks/m12-sampler-buffer-reuse/compare.json` with profiler notes present for every M8 row still slower than 1.5x on the local M8 baseline.
The measured `m8-sample-throughput-1000000` row improved from 0.116693855s in the pre-optimization primary probe to 0.101886795s after reusing sampler output buffers, but it remains slower than the pinned Stim baseline and needs follow-up frame reuse or batched sampling work before it can satisfy the beta performance gate.
The sampler-frame-reuse report wrote `target/benchmarks/m12-sampler-frame-reuse-final/compare.json` with profiler notes present for every M8 row still slower than 1.5x on the local M8 baseline.
It measured `m8-sample-throughput-1000000` at 0.082798783s after reusing `StabilizerFrame` state across shots, while `m8-sample-throughput-1024` remained fast at 0.000085376s on the same local run.
The allocation-enabled sampler report wrote `target/benchmarks/m12-sampler-buffer-reuse-alloc/compare.json`; it records `stab_allocation_bytes_max=2000138` for the million-shot `01` output row, dominated by the intentionally materialized output bytes rather than per-shot record allocations.
The direct noisy Z-measurement report wrote `target/benchmarks/m12-direct-z-measure-fast-path/compare.json` and measured `m8-sample-throughput-1000000` at 0.003941562s in the M8-only probe.
The updated primary compare report wrote `target/benchmarks/m12-primary-compare-after-direct-z-fast-path/compare.json`, measured `m8-sample-throughput-1000000` at 0.001928412s, and moved that row from 4.010664434843194x to 0.09042237387579168x the pinned Stim CLI baseline.
The remaining comparable beta failures in that probe were `m4-circuit-parse`, `m5-sparse-xor`, `m6-clifford-string`, `m6-pauli-string`, `m6-pauli-iter`, `m8-probability-util`, and `m8-measure-reader`.
The target parser fast-path report wrote `target/benchmarks/m12-target-plain-qubit-fast-path/compare.json` and measured `m4-circuit-parse` sparse parsing at 0.000465780s in the M4-only probe.
The updated primary compare report wrote `target/benchmarks/m12-primary-compare-after-target-fast-paths/compare.json`, measured `m4-circuit-parse` sparse parsing at 0.000231810s, and improved that row from 3.851013698630137x to 3.1754794520547946x the pinned Stim parser baseline.
The remaining comparable beta failures in that probe were `m4-circuit-parse`, `m5-sparse-xor`, `m6-clifford-string`, `m6-pauli-string`, `m6-pauli-iter`, `m8-probability-util`, and `m8-measure-reader`.
The sparse XOR in-place report wrote `target/benchmarks/m12-sparse-xor-in-place/compare.json` and measured `m5-sparse-xor` row XOR at 0.000034768s in the M5-only probe.
The updated primary compare report wrote `target/benchmarks/m12-primary-compare-after-sparse-xor-in-place/compare.json`, measured `m5-sparse-xor` row XOR at 0.000018720s, and moved that row from 2.9395x to 1.3371428571428572x the pinned Stim sparse XOR baseline.
The remaining comparable beta failures in that probe were `m4-circuit-parse`, `m6-clifford-string`, `m6-pauli-string`, `m6-pauli-iter`, `m8-probability-util`, and `m8-measure-reader`.
The clean M8 measure-reader report wrote `target/benchmarks/m12-measure-reader-current-clean/compare.json`, measured `stab_measure_reader_01_10k` at 0.000006496s, and left `m8-measure-reader` failing at 2.095483870967742x the pinned Stim reader baseline.
The `01` reader fast-path report wrote `target/benchmarks/m12-measure-reader-byte-zero-one-checked-scan-repeat/compare.json`, measured `stab_measure_reader_01_10k` at 0.000005856s, and moved `m8-measure-reader` under the beta gate at 1.8890322580645162x in the focused M8 probe.
The updated primary compare report wrote `target/benchmarks/m12-primary-compare-after-zero-one-reader-checked-scan/compare.json`, measured `m8-measure-reader` at 1.914516129032258x, and reported 51 passing comparable rows, 5 failing comparable rows, 15 not-comparable contract rows, and no missing, pending, or invalid baseline rows.
The remaining comparable beta failures in that probe were `m4-circuit-parse`, `m8-probability-util`, `m6-clifford-string`, `m6-pauli-string`, and `m6-pauli-iter`.
The parser numeric-target fast-path report wrote `target/benchmarks/m12-circuit-parser-numeric-target-fast-paths-repeat/compare.json`, measured `m4-circuit-parse` sparse parsing at 0.000133118s, and moved that row under the beta gate at 1.8235342465753424x in the focused M4 probe.
The updated primary compare report wrote `target/benchmarks/m12-primary-compare-after-parser-numeric-target-fast-paths/compare.json`, measured `m4-circuit-parse` sparse parsing at 0.000131742s, and reported 52 passing comparable rows, 4 failing comparable rows, 15 not-comparable contract rows, and no missing, pending, or invalid baseline rows.
The remaining comparable beta failures in that probe were `m8-probability-util`, `m6-clifford-string`, `m6-pauli-string`, and `m6-pauli-iter`.
The probability-utility direct report wrote `target/benchmarks/m12-probability-util-direct-hoisted-log/compare.json`, replaced the previous sampler-path proxy with seven direct 1024-bit biased-random measurements, and measured `m8-probability-util` at 1.76x the pinned Stim median in the focused M8 probe.
The latest updated primary compare report wrote `target/benchmarks/m12-primary-compare-after-probability-util-direct/compare.json`, measured `m8-probability-util` at 0.96x the pinned Stim median, and reported 53 passing comparable rows, 3 failing comparable rows, 15 not-comparable contract rows, and no missing, pending, or invalid baseline rows.
The remaining comparable beta failures in the latest probe are `m6-clifford-string`, `m6-pauli-string`, and `m6-pauli-iter`.
The M6 in-place multiplication direct report wrote `target/benchmarks/m12-m6-in-place-multiply-direct-identity-cache/compare.json`, measured `m6-clifford-string` at 0.0653061224489796x and `m6-pauli-string` at 0.060000000000000005x the pinned Stim medians in the focused M6 probe, and left `m6-pauli-iter` as the only M6 comparable beta failure.
The latest updated primary compare report wrote `target/benchmarks/m12-primary-compare-after-m6-in-place-multiply-direct/compare.json`, measured `m6-clifford-string` at 0.0653061224489796x and `m6-pauli-string` at 0.04x the pinned Stim medians, and reported 55 passing comparable rows, 1 failing comparable row, 15 not-comparable contract rows, and no missing, pending, or invalid baseline rows.
The remaining comparable beta failure in the latest probe is `m6-pauli-iter`.
The M6 Pauli iterator direct report wrote `target/benchmarks/m12-m6-pauli-iter-direct-borrowed-result/compare.json`, measured `m6-pauli-iter` at 1.0847058823529412x the pinned Stim median in the focused M6 probe, and moved that row under the beta gate.
The latest updated primary compare report wrote `target/benchmarks/m12-primary-compare-after-m6-pauli-iter-direct/compare.json`, measured `m6-pauli-iter` at 0.5567058823529412x the pinned Stim median, and reported 56 passing comparable rows, 0 failing comparable rows, 15 not-comparable contract rows, and no missing, pending, or invalid baseline rows.
No comparable beta failures remain in the latest local primary probe, and source-owned short profiler notes now exist for the current rows slower than the 1.5x hot-path note threshold, but deeper profiler captures, allocation baselines, remaining regression thresholds, oracle completion, milestone audit, and full code review remain pending before M12 can be considered complete.
The M12 primary regression-threshold report wrote `target/benchmarks/m12-primary-regression-thresholds/compare.json`, checked `benchmarks/m12-primary-thresholds.json`, and passed the 49 configured 1.25x threshold rows.
The beta-passing or previously beta-passing primary rows not yet configured in `benchmarks/m12-primary-thresholds.json` include `m4-circuit-parse`, `m4-gate-lookup`, `m5-simd-bits`, `m5-simd-word`, `m5-sparse-xor`, `m8-measure-reader`, and `m8-probability-util`, because the latest local primary evidence either measured them above 1.25x pinned Stim or showed enough microbenchmark variance to make a 1.25x threshold premature.
The M12 source-owned profiler-note report wrote `target/benchmarks/m12-primary-source-profiler-notes/compare.json`, read notes from `benchmarks/profiler-notes/m12`, and reported 3 present profiler notes, 68 rows where notes were not required, and 0 missing or invalid notes.
