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
Compare prints Stab-side timings for rows whose implementation milestone has a runner and prints pending rows explicitly for future milestones.
When a comparison runner reports workload-specific rates or comparability notes, treat those notes as part of the benchmark evidence.
For example, M5 labels Stab-only contract-smoke bit-kernel workloads separately from upstream Stim perf rows until M12 introduces optimized parity thresholds.
When a row is contract-only, compare may report Stab-side timing with `stim=contract-only`; that is not a Stab-vs-Stim performance comparison for the row.
Pass `--strict` to fail when any selected row is still pending or missing from the selected baseline report.
