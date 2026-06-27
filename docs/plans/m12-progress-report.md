# M12 Progress Report

## Milestone

M12: Performance Hardening

## Status

Partial progress, not milestone-complete.
This slice starts the M12 measurement gate by making the primary benchmark matrix explicit, adding release-profile compare report generation, recording row-level timing, variance, relative-ratio, pass/fail status, and allocation metadata, adding beta-gate enforcement, and adding profiler-note enforcement for rows slower than the hot-path threshold.

## Contract

M12 requires a frozen primary benchmark matrix, a release-profile Stab-vs-pinned-Stim compare command, a report artifact with machine, compiler, Stim, Stab, benchmark-parameter, timing, variance, ratio, and status metadata, profiler notes for slow workloads, targeted optimizations behind existing abstractions, allocation tracking, regression thresholds, and all implemented oracle suites passing before and after performance changes.
This slice covers only the matrix selection, compare-reporting infrastructure, beta-gate enforcement, profiler-note gate infrastructure, and allocation-tracking report plumbing.
Actual profiler captures, optimization work, complete allocation reports for the primary matrix, regression thresholds, beta performance gates, and beta memory gates remain pending M12 work.

## Tests Ported Or Created

- `cargo test -p stab-bench primary_compare_rows_freeze_m4_through_m11_without_metadata_or_m12_placeholders` checks that the M12 primary matrix selects real M4 through M11 workloads and excludes metadata anchors plus the M12 placeholder.
- `cargo test -p stab-bench compare_row_result_records_ratio_and_beta_gate_status` checks that compare rows record medians, relative ratios, notes, and pass status for rows within the 2.0x beta gate.
- `cargo test -p stab-bench beta_gate_requires_every_selected_row_to_prove_a_pass` checks that completion-style beta-gate enforcement rejects rows that are missing comparable evidence or exceed 2.0x.
- `cargo test -p stab-bench compare_row_result_distinguishes_missing_baseline_from_uncomparable_contracts` checks that missing baselines and contract-only rows are not collapsed into the same status.
- `cargo test -p stab-bench profiler_notes_are_required_only_for_rows_slower_than_hot_path_ratio` checks that rows above 1.5x require note files and rows at or below the threshold do not.
- `cargo test -p stab-bench profiler_notes_must_name_dominant_cost_and_next_owner_action` checks the minimum required profiler-note fields.
- `cargo test -p stab-bench benchmark_ids_are_filename_safe_for_report_artifacts` checks that benchmark ids are safe to map into profiler-note filenames.
- `cargo test -p stab-bench compare_row_result_records_stab_allocation_maxima` checks that per-measurement allocation data is promoted to row-level compare report fields.
- `cargo test -p stab-bench allocation_tracking_guard_requires_count_allocations_feature` checks that `--track-allocations` is only available in the allocation-enabled build.
- `cargo test -p stab-bench --features count-allocations` checks that the optional allocation-counting build compiles and runs the benchmark ops tests.
- Existing `cargo test -p stab-bench` coverage still validates benchmark manifest structure, baseline metadata validation, Stab comparison runner coverage, and benchmark output path guards.

## Implementation Areas

- Added `BenchmarkManifest::compare_rows` and `BenchmarkRow::is_primary` in `ops/bench/src/manifest.rs` so primary-matrix selection is explicit and test-covered.
- Added `--profile`, `--primary`, and `--report` to `stab-bench compare`.
- Added `--require-beta-gate` so completion-style compare runs fail unless every selected row proves the 2.0x pinned-Stim beta performance gate.
- Added `--require-profiler-notes` to enforce profiler notes for rows slower than 1.5x pinned Stim when a compare report is written.
- Updated `just bench::compare` to run `stab-bench` through Cargo's release profile before invoking the compare subcommand.
- Moved compare orchestration into `ops/bench/src/compare.rs` so `ops/bench/src/baseline.rs` remains below the repository's 1200-line source threshold.
- Added `CompareReport`, `CompareRowResult`, `CompareCommandMetadata`, and `StabMetadata` in `ops/bench/src/report.rs`, including the Stab commit and whether local modifications were present when the report was generated.
- Added compare artifact writing to `target/benchmarks/.../compare.json` and `target/benchmarks/.../report.md` through the existing benchmark output directory guard.
- Added profiler-note status, path, and error fields to each compare row; notes live under `<report>/profiler-notes/<benchmark-id>.md` and must include non-empty `Dominant cost:` and `Next owner action:` lines.
- Added the optional `count-allocations` feature using the existing `allocation-counter` crate rather than introducing a hand-written allocator in this workspace.
- Added `--track-allocations` and `just bench::compare-allocations` so Stab-side allocation counts can be recorded separately from timing-gate runs.
- Extended `Measurement` with optional allocation totals and compare rows with Stab allocation max fields.
- Extended `Measurement` with optional `variance_seconds`; existing baseline JSON remains readable because the new field defaults when absent.
- Tightened benchmark manifest validation so benchmark ids are safe for generated report artifact filenames.
- Updated root and benchmark documentation for the new compare command and artifact behavior.

## Done-Criteria Matrix

| Requirement | Status | Evidence |
| --- | --- | --- |
| Freeze primary benchmark matrix from earlier milestones | Partially satisfied | `BenchmarkRow::is_primary`, `BenchmarkManifest::compare_rows`, and `cargo test -p stab-bench primary_compare_rows_freeze_m4_through_m11_without_metadata_or_m12_placeholders`; final workload acceptance remains pending M12 audit after baseline/report runs. |
| Add `just bench::compare --profile release --report target/benchmarks/latest` | Partially satisfied | `justfiles/bench.just` now builds `stab-bench` with Cargo's release profile, and `stab-bench compare` accepts `--profile`, `--primary`, and `--report`; a durable full primary report against a complete pinned-Stim baseline remains pending. |
| Report machine, compiler, Stim, Stab, benchmark parameters, median timing, variance, ratio, and status | Partially satisfied | `CompareReport` records machine metadata, Stim metadata, Stab commit and local-modification state, compare command metadata, per-row measurements, medians, optional variance, optional allocation data, relative ratio, pass/fail status, beta-gate status, and profiler-note status; complete primary allocation reports remain pending. |
| Profile slower-than-gate workloads before optimizing | Partially satisfied | `--require-profiler-notes`, `profiler_notes_are_required_only_for_rows_slower_than_hot_path_ratio`, and `profiler_notes_must_name_dominant_cost_and_next_owner_action` enforce durable note files for rows slower than 1.5x once a complete report exists; actual profiler captures and notes for current slow rows remain pending. |
| Optimize hot paths behind existing abstractions | Missing | No hot-path optimization is performed in this slice. |
| Add allocation tracking for primary hot paths | Partially satisfied | `--track-allocations`, `just bench::compare-allocations`, optional `count-allocations`, and `compare_row_result_records_stab_allocation_maxima` provide Stab-side allocation-count plumbing; a full primary allocation report and memory-regression comparison remain pending. |
| Add regression thresholds for workloads that pass the beta gate | Missing | Threshold enforcement remains pending complete baseline and compare reports. |
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
- `cargo clippy -p stab-bench --all-targets -- -D warnings`
- `cargo clippy -p stab-bench --features count-allocations --all-targets -- -D warnings`
- `just bench::smoke`
- `just bench::compare --help`
- `just bench::compare --milestone M4 --report target/benchmarks/m12-compare-smoke --require-profiler-notes`
- `cargo run -q -p stab-bench -- compare --milestone M4 --require-beta-gate; rc=$?; echo rc=$rc; test $rc -ne 0`
- `just bench::compare-allocations --milestone M4 --report target/benchmarks/m12-alloc-smoke`

The M4 compare-report smoke wrote `compare.json` and `report.md` successfully.
The local baseline at `target/benchmarks/baseline/latest/baseline.json` did not include M4 rows, so the smoke report correctly marked those rows as `missing-baseline`; a complete primary baseline remains required before M12 can satisfy the beta performance gate.
Because those rows had no Stim baseline measurements, the smoke report had no relative ratios and correctly marked profiler notes as `not-required`.
The beta-gate smoke command failed as expected because the local baseline has missing M4 rows, proving the enforcement path rejects unproven rows.
The allocation smoke wrote allocation counts and maximum live allocated bytes for the M4 Stab-side measurements.
