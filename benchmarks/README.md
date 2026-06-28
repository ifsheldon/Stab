# Benchmark Contracts

`manifest.csv` is the source-owned benchmark contract manifest for M3 and later performance work.
Each row names the owning implementation milestone, threshold class, runner, upstream Stim source, workload phase, and measurement family.
`just bench::smoke` validates that required M3 contracts remain present, including the primary benchmark matrix, canonical print and convert contracts, bit-packed detector conversion contracts, and the M12 performance-hardening target.

Runner meanings:

- `stim-perf`: run pinned C++ Stim's `stim_perf` binary with the row's `stim_perf_filter`.
- `stim-cli`: run pinned C++ Stim's `stim` binary with the row's pipe-delimited `argv` and optional `stdin_path`.
- `contract-only`: reserve a benchmark contract that has no direct pinned C++ executable baseline yet.

Some legacy benchmark ids still contain `contract` after they gain a faithful public CLI baseline.
The `runner` column is authoritative for whether a row is contract-only, not the id suffix.

Generated benchmark artifacts belong under `target/benchmarks/` and are not source files.
The default baseline command writes `target/benchmarks/baseline/latest/baseline.json` and `target/benchmarks/baseline/latest/report.md`.
Any explicit `--out` value must be a repository-relative path under `target/benchmarks/`.
Use `--only` with exact benchmark ids or milestone names, for example `--only m4-circuit-parse` or `--only M9`.
Pass `--primary` to record only the frozen M12 primary matrix, using the same M4 through M11 row selection as `just bench::compare --primary`.

`just bench::compare` reads `target/benchmarks/baseline/latest/baseline.json` by default.
Pass `--baseline <path>` to compare against a different generated baseline report.
Pass `--primary` to select the frozen M12 primary matrix, which currently includes M4 through M11 benchmark rows except metadata anchors and the M12 placeholder row.
Pass `--profile release` to record the intended Cargo profile in compare output; the `just bench::compare` recipe builds the benchmark ops binary with Cargo's release profile before running the subcommand.
Pass `--report target/benchmarks/latest` or another repository-relative directory below `target/benchmarks/` to write `compare.json` and `report.md`.
Compare row ratios use paired measurement ratios when comparable submeasurements are available.
Every compare row has a machine-readable comparability class recorded in `compare.json` as `comparability`.
The class is derived from the source-owned compare-note prefix or from a `contract-only` runner with no note.
Supported classes are `direct-match`, `cli-baseline`, `contract-representative`, `contract-proxy`, `contract-smoke`, `partial-match`, `report-only`, and `contract-only`.
`direct-match` rows match pinned Stim operation shape closely enough to use exact-name submeasurement pairs or positional submeasurement pairs when counts match.
`cli-baseline` rows compare Stab's implementation of the same public CLI command, input, and output contract against pinned Stim's public CLI.
`contract-representative`, `contract-proxy`, `contract-smoke`, `partial-match`, and `report-only` rows are narrower M12 beta evidence classes; their note must explain the missing exact parity or representative scope before the row is treated as reviewable benchmark evidence.
`contract-only` rows do not prove a Stab-vs-Stim timing ratio and require a source-owned beta waiver when they are selected by `--require-beta-gate`.
Pairs are matched by normalized measurement names, or by position for `direct-match` rows whose Stim and Stab measurement counts match.
When paired ratios exist, `direct-match` and `cli-baseline` gates use the worse of the row median ratio and the worst paired ratio; `partial-match` gates use the worst paired ratio so unmatched Stab contract extras remain visible without deciding a Stim-relative gate; rows without paired evidence use the row median ratio.
The JSON report records paired evidence in `measurement_ratios`, and the Markdown report prints the worst pair in the `Ratio Source` column.
Tiny direct-match Stab measurements may use batched timing to reduce clock-noise dominance, but they still report seconds per operation.
Pass `--require-beta-gate` to fail when any selected row does not prove a pass against the 1.25x pinned-Stim beta performance gate.
Pass `--beta-waivers <path>` with `--require-beta-gate` to accept only measured no-ratio rows whose manifest `runner` is `contract-only` and whose lack of a comparable pinned-Stim ratio is explained by a source-owned waiver.
Waivers do not apply to missing baselines, pending runners, invalid baselines, or rows with measured ratios above the beta gate.
Unused waivers fail the gate so the file stays in sync when a row becomes comparable or leaves the selected matrix.
`m12-primary-beta-waivers.json` is the source-owned M12 waiver file for the remaining primary rows that have Stab-side timing evidence but no faithful public Stim baseline.
Run `just bench::primary-beta --baseline <primary-baseline.json>` to check the M12 beta timing gate with source-owned profiler notes and waivers.
Pass `--require-profiler-notes` with `--report` to fail when a row slower than 1.5x pinned Stim lacks a valid note at `<report>/profiler-notes/<benchmark-id>.md`.
Profiler notes must include non-empty `Dominant cost:` and `Next owner action:` lines.
Pass `--profiler-notes-dir benchmarks/profiler-notes/m12` to validate source-owned notes instead of report-local notes.
M12 rows optimized during performance hardening are tracked in `profiler-notes/m12/optimization-log.json`.
Optimization-log rows use schema version 2 and record before and after reports, source-owned before and after ratios, gate status, hot-path status, source profiler-note paths for after rows still above the 1.5x profiler-note threshold, dominant-cost evidence, implementation summary, semantic checks, and follow-up policy.
Pass `--thresholds <path>` to fail when a selected row with a configured regression threshold exceeds its maximum relative ratio or lacks a comparable Stab-vs-Stim ratio.
Threshold files must not contain stale ids: every configured threshold id must be selected by the compare run so row renames and matrix changes cannot silently drop a regression guard.
`m12-primary-thresholds.json` is the source-owned M12 timing-regression threshold file for primary rows that have reached the 1.25x pinned-Stim regression gate with enough local headroom to make an initial stable threshold useful.
Run `just bench::primary-regression --baseline <primary-baseline.json> --report target/benchmarks/<name>` to check those source-owned thresholds and checked timing-regression waivers for the frozen primary matrix after a Stab-side warmup pass, three recorded measurement runs, and source-owned profiler-note validation.
The recipe defaults to the latest generated baseline path when no explicit `--baseline` is passed.
The scheduled `.github/workflows/m12-benchmarks.yml` workflow records a fresh primary pinned-Stim baseline on a GitHub runner, runs this source-owned threshold gate, and uploads the generated baseline and compare reports.
Threshold files are JSON with schema version 1 or 2.
Schema version 1 rows use only row-level thresholds:

```json
{
  "schema_version": 1,
  "rows": [
    {
      "id": "m4-circuit-parse",
      "max_relative_ratio": 1.25
    }
  ]
}
```

Every threshold id must match a selected benchmark row, and every selected benchmark row not present in the threshold file is reported as `not-configured`.
If `--regression-waivers <path>` is also passed, selected measured `contract-only` rows that are not configured in the threshold file, have no comparable ratio, and have source-owned waiver entries are reported as `waived-not-thresholdable` instead of ambiguous `not-configured`.
Timing-regression waivers do not apply to comparable rows, rows with ratios, unselected rows, pending rows, or configured threshold rows, and unused waivers fail the gate so the waiver file cannot drift when a row becomes comparable.
`m12-primary-regression-waivers.json` is the source-owned M12 timing-regression waiver file for primary no-ratio rows that are measured but cannot have a faithful pinned-Stim 1.25x threshold.
Schema version 2 is backward compatible with row-level thresholds and additionally supports exact submeasurement thresholds for rows whose stable direct measurements can be guarded before the whole mixed row is stable:

```json
{
  "schema_version": 2,
  "rows": [
    {
      "id": "m10-error-decomp",
      "measurement_thresholds": [
        {
          "stim_name": "disjoint_to_independent_xyz_errors_approx_p10",
          "stab_name": "stab_disjoint_to_independent_xyz_errors_approx_p10",
          "max_relative_ratio": 1.25
        }
      ]
    }
  ]
}
```

Submeasurement thresholds fail when the selected compare report lacks the named paired evidence or when the paired ratio exceeds the configured ratio.
The timing-regression report is the authoritative completion evidence for configured schema-version-2 threshold pairs because threshold application materializes any configured explicit pair that was not already produced by automatic exact-name or positional pairing.
Threshold ids must be benchmark-id safe because they are matched against report rows and may also be used by generated benchmark tooling.
Beta and timing-regression waiver files are JSON with schema version 1:

```json
{
  "schema_version": 1,
  "rows": [
    {
      "id": "m4-circuit-canonical-print",
      "reason": "Pinned Stim has no public comparable baseline for this exact workload.",
      "follow_up": "Replace the waiver if a faithful baseline becomes available."
    }
  ]
}
```

Waiver ids must be benchmark-id safe.
Reasons and follow-up text must be non-empty because they are copied into the compare report as durable beta-gate evidence.
Use `just bench::compare-allocations` to build `stab-bench` with the optional `count-allocations` feature and pass `--track-allocations` automatically.
Allocation tracking runs an extra Stab-side measurement pass per reported measurement and records allocation counts, maximum live allocated bytes, and sampled resident bytes in `compare.json`; use plain `just bench::compare` for timing-gate evidence.
Pass `--require-memory-gate --memory-baseline <compare.json>` with `just bench::compare-allocations` to compare selected rows against the first complete Stab memory report.
The memory gate fails rows missing current or baseline allocation bytes, rows missing current or baseline resident bytes, rows whose `stab_allocation_bytes_max` exceeds the baseline by more than 25 percent, and rows whose `stab_resident_bytes_max` exceeds the baseline by more than 25 percent.
`m12-primary-memory-baseline.json` is the source-owned M12 memory-regression baseline for the frozen primary matrix and records both `stab_allocation_bytes_max` and `stab_resident_bytes_max`.
Run `just bench::primary-memory-regression --baseline <primary-baseline.json>` to check the source-owned memory baseline with allocation and resident-memory tracking, profiler-note validation, and a report at `target/benchmarks/m12-primary-memory-regression`.
Compare prints Stab-side timings for rows whose implementation milestone has a runner and prints pending rows explicitly for future milestones.
When a comparison runner reports workload-specific rates or comparability notes, treat those notes as part of the benchmark evidence.
For example, M5 labels Stab-only contract-smoke bit-kernel workloads separately from upstream Stim perf rows until M12 introduces optimized parity thresholds.
M8 sample compare rows split Stab core sampler compilation, one-shot latency, and batch throughput in-process; those report-only rows are not a strict CLI-vs-CLI performance gate, and the probability-util row measures the direct Stab biased-random utility API against pinned Stim's probability utility perf filters.
When a row is contract-only, compare may report Stab-side timing with `stim=contract-only`; that is not a Stab-vs-Stim performance comparison for the row.
Pass `--strict` to fail when any selected row is still pending, missing from the selected baseline report, backed by an invalid placeholder baseline row, contract-only without a Stab-side measurement, or backed by baseline metadata that does not match pinned Stim v1.16.0.
