# Benchmark Contracts

`manifest.csv` is the source-owned benchmark contract manifest for M3 and later performance work.
Each row names the owning implementation milestone, threshold class, runner, upstream Stim source, workload phase, and measurement family.
`just bench::smoke` validates that required M3 contracts remain present, including the primary benchmark matrix, canonical print and convert contracts, bit-packed detector conversion contracts, and the M12 performance-hardening target.

Runner meanings:

- `stim-perf`: run pinned C++ Stim's `stim_perf` binary with the row's `stim_perf_filter`.
- `stim-cli`: run pinned C++ Stim's `stim` binary with the row's pipe-delimited `argv` and optional `stdin_path`.
- `contract-only`: reserve a benchmark contract that has no direct pinned C++ executable baseline yet.

Generated benchmark artifacts belong under `target/benchmarks/` and are not source files.
The default baseline command writes `target/benchmarks/baseline/latest/baseline.json` and `target/benchmarks/baseline/latest/report.md`.
Any explicit `--out` value must be a repository-relative path under `target/benchmarks/`.
Use `--only` with exact benchmark ids or milestone names, for example `--only m4-circuit-parse` or `--only M9`.

`just bench::compare` reads `target/benchmarks/baseline/latest/baseline.json` by default.
Pass `--baseline <path>` to compare against a different generated baseline report.
Pass `--primary` to select the frozen M12 primary matrix, which currently includes M4 through M11 benchmark rows except metadata anchors and the M12 placeholder row.
Pass `--profile release` to record the intended Cargo profile in compare output; the `just bench::compare` recipe builds the benchmark ops binary with Cargo's release profile before running the subcommand.
Pass `--report target/benchmarks/latest` or another repository-relative directory below `target/benchmarks/` to write `compare.json` and `report.md`.
Pass `--require-beta-gate` to fail when any selected row does not prove a pass against the 2.0x pinned-Stim beta performance gate.
Pass `--require-profiler-notes` with `--report` to fail when a row slower than 1.5x pinned Stim lacks a valid note at `<report>/profiler-notes/<benchmark-id>.md`.
Profiler notes must include non-empty `Dominant cost:` and `Next owner action:` lines.
Pass `--thresholds <path>` to fail when a selected row with a configured regression threshold exceeds its maximum relative ratio or lacks a comparable Stab-vs-Stim ratio.
Threshold files are JSON with schema version 1:

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

Only selected benchmark ids present in the threshold file are checked.
Threshold ids must be benchmark-id safe because they are matched against report rows and may also be used by generated benchmark tooling.
Use `just bench::compare-allocations` to build `stab-bench` with the optional `count-allocations` feature and pass `--track-allocations` automatically.
Allocation tracking runs an extra Stab-side measurement pass per reported measurement and records allocation counts and maximum live allocated bytes in `compare.json`; use plain `just bench::compare` for timing-gate evidence.
Compare prints Stab-side timings for rows whose implementation milestone has a runner and prints pending rows explicitly for future milestones.
When a comparison runner reports workload-specific rates or comparability notes, treat those notes as part of the benchmark evidence.
For example, M5 labels Stab-only contract-smoke bit-kernel workloads separately from upstream Stim perf rows until M12 introduces optimized parity thresholds.
M8 sample compare rows split Stab core sampler compilation, one-shot latency, and batch throughput in-process; those report-only rows are not a strict CLI-vs-CLI performance gate, and the probability-util row currently exercises the sampler probability path until Stab has a standalone biased-random utility API.
When a row is contract-only, compare may report Stab-side timing with `stim=contract-only`; that is not a Stab-vs-Stim performance comparison for the row.
Pass `--strict` to fail when any selected row is still pending, missing from the selected baseline report, backed by an invalid placeholder baseline row, contract-only without a Stab-side measurement, or backed by baseline metadata that does not match pinned Stim v1.16.0.
