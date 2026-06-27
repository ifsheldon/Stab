# M12 Progress Report

## Milestone

M12: Performance Hardening

## Status

Partial progress, not milestone-complete.
This work starts the M12 measurement gate by making the primary benchmark matrix explicit, adding release-profile compare report generation, recording row-level timing, variance, relative-ratio, pass/fail status, and allocation metadata, adding beta-gate enforcement, adding profiler-note enforcement for rows slower than the hot-path threshold, adding regression-threshold enforcement infrastructure, adding beta memory-gate enforcement infrastructure, and optimizing the first sampler output hot path.

## Contract

M12 requires a frozen primary benchmark matrix, a release-profile Stab-vs-pinned-Stim compare command, a report artifact with machine, compiler, Stim, Stab, benchmark-parameter, timing, variance, ratio, and status metadata, profiler notes for slow workloads, targeted optimizations behind existing abstractions, allocation tracking, regression thresholds, and all implemented oracle suites passing before and after performance changes.
This work covers only the matrix selection, compare-reporting infrastructure, beta-gate enforcement, profiler-note gate infrastructure, allocation-tracking report plumbing, regression-threshold file enforcement, memory-gate compare-report enforcement, and a focused `CompiledSampler::sample_bytes` allocation/output-buffer optimization.
Broader profiler captures, optimization work across parser, sampler, detector, analyzer, and DEM hot paths, complete allocation reports for the primary matrix, source-owned per-row regression threshold values, beta performance gates, and a source-owned first complete memory baseline remain pending M12 work.

## Tests Ported Or Created

- `cargo test -p stab-bench primary_compare_rows_freeze_m4_through_m11_without_metadata_or_m12_placeholders` checks that the M12 primary matrix selects real M4 through M11 workloads and excludes metadata anchors plus the M12 placeholder.
- `cargo test -p stab-bench compare_row_result_records_ratio_and_beta_gate_status` checks that compare rows record medians, relative ratios, notes, and pass status for rows within the 2.0x beta gate.
- `cargo test -p stab-bench beta_gate_requires_every_selected_row_to_prove_a_pass` checks that completion-style beta-gate enforcement rejects rows that are missing comparable evidence or exceed 2.0x.
- `cargo test -p stab-bench compare_row_result_distinguishes_missing_baseline_from_uncomparable_contracts` checks that missing baselines and contract-only rows are not collapsed into the same status.
- `cargo test -p stab-bench profiler_notes_are_required_only_for_rows_slower_than_hot_path_ratio` checks that rows above 1.5x require note files and rows at or below the threshold do not.
- `cargo test -p stab-bench profiler_notes_must_name_dominant_cost_and_next_owner_action` checks the minimum required profiler-note fields.
- `cargo test -p stab-bench regression_thresholds_mark_pass_fail_and_uncomparable_rows` checks that configured threshold rows pass, fail, or become blocking not-comparable rows based on their relative ratio.
- `cargo test -p stab-bench regression_thresholds_validate_schema_ids_and_ratios` checks that threshold files reject unsupported schema versions, unsafe benchmark ids, duplicate ids, and invalid ratio values.
- `cargo test -p stab-bench benchmark_ids_are_filename_safe_for_report_artifacts` checks that benchmark ids are safe to map into profiler-note filenames.
- `cargo test -p stab-bench compare_row_result_records_stab_allocation_maxima` checks that per-measurement allocation data is promoted to row-level compare report fields.
- `cargo test -p stab-bench allocation_tracking_guard_requires_count_allocations_feature` checks that `--track-allocations` is only available in the allocation-enabled build.
- `cargo test -p stab-bench memory_gate_marks_pass_fail_and_missing_allocation_rows` checks that the memory gate passes rows within 25 percent, fails rows over budget, and blocks missing baseline or current allocation evidence.
- `cargo test -p stab-bench memory_gate_rejects_unsupported_baseline_schema` checks that memory baselines must use compare-report schema version 1.
- `cargo test -p stab-core sampling seeded_sample_bytes_match_seeded_record_samples` checks that the optimized streaming-byte sampler path still matches the seeded record-sample path for `01` and `b8` outputs.
- `cargo test -p stab-bench --features count-allocations` checks that the optional allocation-counting build compiles and runs the benchmark ops tests.
- Existing `cargo test -p stab-bench` coverage still validates benchmark manifest structure, baseline metadata validation, Stab comparison runner coverage, and benchmark output path guards.

## Implementation Areas

- Added `BenchmarkManifest::compare_rows` and `BenchmarkRow::is_primary` in `ops/bench/src/manifest.rs` so primary-matrix selection is explicit and test-covered.
- Added `--profile`, `--primary`, and `--report` to `stab-bench compare`.
- Added `--require-beta-gate` so completion-style compare runs fail unless every selected row proves the 2.0x pinned-Stim beta performance gate.
- Added `--require-profiler-notes` to enforce profiler notes for rows slower than 1.5x pinned Stim when a compare report is written.
- Added `--thresholds` to enforce JSON schema version 1 regression thresholds against selected benchmark rows and report threshold status per row.
- Updated `just bench::compare` to run `stab-bench` through Cargo's release profile before invoking the compare subcommand.
- Moved compare orchestration into `ops/bench/src/compare.rs` so `ops/bench/src/baseline.rs` remains below the repository's 1200-line source threshold.
- Added `CompareReport`, `CompareRowResult`, `CompareCommandMetadata`, and `StabMetadata` in `ops/bench/src/report.rs`, including the Stab commit and whether local modifications were present when the report was generated.
- Added compare artifact writing to `target/benchmarks/.../compare.json` and `target/benchmarks/.../report.md` through the existing benchmark output directory guard.
- Added profiler-note status, path, and error fields to each compare row; notes live under `<report>/profiler-notes/<benchmark-id>.md` and must include non-empty `Dominant cost:` and `Next owner action:` lines.
- Added regression-threshold status, maximum ratio, and error fields to each compare row so reports explain threshold failures without requiring console logs.
- Added the optional `count-allocations` feature using the existing `allocation-counter` crate rather than introducing a hand-written allocator in this workspace.
- Added `--track-allocations` and `just bench::compare-allocations` so Stab-side allocation counts can be recorded separately from timing-gate runs.
- Added `--require-memory-gate` and `--memory-baseline` so allocation-enabled compare runs can fail rows that exceed the 25 percent M12 peak-live-allocation regression budget.
- Extended `Measurement` with optional allocation totals and compare rows with Stab allocation max fields.
- Added memory-gate status, baseline bytes, allowed bytes, and error fields to compare rows so memory-gate failures are preserved in generated reports.
- Extended `Measurement` with optional `variance_seconds`; existing baseline JSON remains readable because the new field defaults when absent.
- Added `CompiledSampler` measurement-count tracking, output-capacity reservation, and reusable per-shot record/output buffers for `sample_bytes`, reducing allocation churn in high-shot `01` and `b8` sampling.
- Added `MeasureRecordWriter::with_capacity` so hot sampling and future writers can reserve output storage without changing the existing writer behavior.
- Tightened benchmark manifest validation so benchmark ids are safe for generated report artifact filenames.
- Updated root and benchmark documentation for the new compare command and artifact behavior.

## Done-Criteria Matrix

| Requirement | Status | Evidence |
| --- | --- | --- |
| Freeze primary benchmark matrix from earlier milestones | Partially satisfied | `BenchmarkRow::is_primary`, `BenchmarkManifest::compare_rows`, and `cargo test -p stab-bench primary_compare_rows_freeze_m4_through_m11_without_metadata_or_m12_placeholders`; final workload acceptance remains pending M12 audit after baseline/report runs. |
| Add `just bench::compare --profile release --report target/benchmarks/latest` | Partially satisfied | `justfiles/bench.just` now builds `stab-bench` with Cargo's release profile, and `stab-bench compare` accepts `--profile`, `--primary`, and `--report`; a durable full primary report against a complete pinned-Stim baseline remains pending. |
| Report machine, compiler, Stim, Stab, benchmark parameters, median timing, variance, ratio, and status | Partially satisfied | `CompareReport` records machine metadata, Stim metadata, Stab commit and local-modification state, compare command metadata, per-row measurements, medians, optional variance, optional allocation data, relative ratio, pass/fail status, beta-gate status, memory-gate status, and profiler-note status; complete primary allocation reports remain pending. |
| Profile slower-than-gate workloads before optimizing | Partially satisfied | `--require-profiler-notes`, `profiler_notes_are_required_only_for_rows_slower_than_hot_path_ratio`, and `profiler_notes_must_name_dominant_cost_and_next_owner_action` enforce durable note files for rows slower than 1.5x once a complete report exists; actual profiler captures and notes for current slow rows remain pending. |
| Optimize hot paths behind existing abstractions | Partially satisfied | `CompiledSampler::sample_bytes` now reuses per-shot buffers and reserves `01` and `b8` output capacity; `cargo test -p stab-core sampling seeded_sample_bytes_match_seeded_record_samples` preserves output semantics. The local M8 probe improved `m8-sample-throughput-1000000` from 0.116693855s in `target/benchmarks/m12-primary-probe/compare.json` to 0.101886795s in `target/benchmarks/m12-sampler-buffer-reuse/compare.json`; many other primary hot paths remain unoptimized. |
| Add allocation tracking for primary hot paths | Partially satisfied | `--track-allocations`, `just bench::compare-allocations`, optional `count-allocations`, `compare_row_result_records_stab_allocation_maxima`, `--require-memory-gate`, and `memory_gate_marks_pass_fail_and_missing_allocation_rows` provide Stab-side allocation-count plumbing and memory-regression comparison; a full primary allocation report and source-owned baseline remain pending. |
| Add regression thresholds for workloads that pass the beta gate | Partially satisfied | `--thresholds`, `regression_thresholds_mark_pass_fail_and_uncomparable_rows`, and `regression_thresholds_validate_schema_ids_and_ratios` provide the gate and schema validation; source-owned threshold rows remain pending a complete primary report with passing workloads. |
| `just oracle::run --implemented-only` passes before and after performance changes | Not applicable | This slice changes benchmark ops and docs only; no performance implementation changed, but the oracle gate remains required before M12 completion. |
| `just bench::compare --profile release --primary` has no missing primary workloads | Missing | `--require-beta-gate` now enforces comparable passing rows, but a full primary compare requires a complete pinned-Stim baseline and remains pending. |

## Audit And Review Notes

- Milestone audit has not been run for M12 because the milestone is intentionally incomplete.
- Full code review has not been run for M12 because the milestone is intentionally incomplete.
- No under-specification findings were logged in this slice.

## Verification Commands

- `cargo fmt --all`
- `cargo test -p stab-bench --quiet`
- `cargo test -p stab-bench --features count-allocations --quiet`
- `cargo test -p stab-core sampling --quiet`
- `cargo test -p stab-cli sample --quiet`
- `cargo clippy -p stab-bench --all-targets -- -D warnings`
- `cargo clippy -p stab-bench --features count-allocations --all-targets -- -D warnings`
- `cargo clippy -p stab-core --all-targets -- -D warnings`
- `cargo clippy -p stab-cli --all-targets -- -D warnings`
- `just bench::smoke`
- `just oracle::run --milestone M8`
- `just bench::compare --help`
- `just bench::compare --primary --report target/benchmarks/m12-primary-probe`
- `just bench::compare --milestone M8 --report target/benchmarks/m12-sampler-buffer-reuse --require-profiler-notes`
- `just bench::compare-allocations --milestone M8 --report target/benchmarks/m12-sampler-buffer-reuse-alloc`
- `just bench::compare --milestone M4 --report target/benchmarks/m12-compare-smoke --require-profiler-notes`
- `cargo run -q -p stab-bench -- compare --milestone M4 --require-beta-gate; rc=$?; echo rc=$rc; test $rc -ne 0`
- `cargo run -q -p stab-bench -- compare --milestone M4 --thresholds target/benchmarks/m12-threshold-smoke/thresholds.json; rc=$?; echo rc=$rc; test $rc -ne 0`
- `cargo run -q -p stab-bench --features count-allocations -- compare --track-allocations --milestone M4 --require-memory-gate --memory-baseline target/benchmarks/m12-memory-smoke/compare.json; rc=$?; echo rc=$rc; test $rc -ne 0`
- `just bench::compare-allocations --milestone M4 --report target/benchmarks/m12-alloc-smoke`

The M4 compare-report smoke wrote `compare.json` and `report.md` successfully.
The local baseline at `target/benchmarks/baseline/latest/baseline.json` did not include M4 rows, so the smoke report correctly marked those rows as `missing-baseline`; a complete primary baseline remains required before M12 can satisfy the beta performance gate.
Because those rows had no Stim baseline measurements, the smoke report had no relative ratios and correctly marked profiler notes as `not-required`.
The beta-gate smoke command failed as expected because the local baseline has missing M4 rows, proving the enforcement path rejects unproven rows.
The regression-threshold smoke command failed as expected because the selected threshold row lacked a comparable ratio, proving the threshold gate rejects unproven configured rows.
The memory-gate smoke command failed as expected because the generated memory baseline either lacked selected rows or set a stricter allocation budget than the current run, proving the memory gate rejects missing or over-budget allocation evidence.
The allocation smoke wrote allocation counts and maximum live allocated bytes for the M4 Stab-side measurements.
The sampler-buffer-reuse report wrote `target/benchmarks/m12-sampler-buffer-reuse/compare.json` with profiler notes present for every M8 row still slower than 1.5x on the local M8 baseline.
The measured `m8-sample-throughput-1000000` row improved from 0.116693855s in the pre-optimization primary probe to 0.101886795s after reusing sampler output buffers, but it remains slower than the pinned Stim baseline and needs follow-up frame reuse or batched sampling work before it can satisfy the beta performance gate.
The allocation-enabled sampler report wrote `target/benchmarks/m12-sampler-buffer-reuse-alloc/compare.json`; it records `stab_allocation_bytes_max=2000138` for the million-shot `01` output row, dominated by the intentionally materialized output bytes rather than per-shot record allocations.
