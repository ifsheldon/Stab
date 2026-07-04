# RPF7 CLI Parity Progress Report

## Implemented Slice: Public CLI Benchmark And Legacy Dispatch Evidence

Stab now has source-owned report-only benchmark coverage for the PF7 public CLI `m2d --sweep` packed `b8` path, the public CLI `m2d --ran_without_feedback` path, the public CLI `analyze_errors --decompose_errors` path, the public CLI generated-circuit `analyze_errors` path, and accepted legacy `--gen` dispatch.
It also has executable CLI evidence for selected legacy-mode conflicts and explicit `--detector_hypergraph` exclusion.

This slice reuses source-owned M9 and M10 fixtures or generated workloads and routes through `stab_cli::run_from`, so it measures public CLI behavior instead of lower-level conversion helpers.

## Evidence

Benchmark row:

- `pf7-cli-m2d-sweep-b8` now has a non-primary report-only runner named `stab_pf7_cli_m2d_sweep_b8`, normalized as shots per second.
- `pf7-cli-m2d-feedback-inline` now has a non-primary report-only runner named `stab_pf7_cli_m2d_feedback_inline`, normalized as shots per second.
- `pf7-cli-analyze-errors-decompose` now has a non-primary report-only runner named `stab_pf7_cli_analyze_errors_decompose`, normalized as circuits per second.
- `pf7-cli-analyze-errors-generated` now has a non-primary report-only runner named `stab_pf7_cli_analyze_errors_generated`, normalized as detectors per second on the source-owned d3/r3 rotated-memory-z generated analyzer workload.
- `pf7-cli-legacy-dispatch-startup` now has a non-primary report-only runner named `stab_pf7_cli_legacy_gen_d3_r3`, normalized as dispatches per second.
- Earlier local probe command `just bench::compare --only pf7-cli-m2d-sweep-b8 --only pf7-cli-m2d-feedback-inline --only pf7-cli-analyze-errors-decompose --only pf7-cli-legacy-dispatch-startup --baseline target/benchmarks/pf7-cli-all-probe-baseline/baseline.json --report target/benchmarks/pf7-cli-all-probe-compare` measured `stab_pf7_cli_m2d_sweep_b8=0.000063024s`, or approximately `7.933e4 shots/s`, `stab_pf7_cli_m2d_feedback_inline=0.000071343s`, or approximately `8.410e4 shots/s`, `stab_pf7_cli_analyze_errors_decompose=0.000034352s`, or approximately `2.911e4 circuits/s`, and `stab_pf7_cli_legacy_gen_d3_r3=0.000043936s`, or approximately `2.276e4 dispatches/s`, as report-only evidence on the local machine. The generated analyzer row has runner coverage but no fresh timing probe recorded in this report yet.

Oracle rows:

- `pf7-legacy-dispatch-conflicts-rust` runs selected legacy conflict cases for `--convert`, `--sample`, `--detect`, `--m2d`, `--analyze_errors`, and `--gen=...`, checking nonzero status, empty stdout, and diagnostic stderr.
- `pf7-detector-hypergraph-excluded-rust` proves deprecated `--detector_hypergraph` is not accepted as a mode and is not exposed as a help topic.

## Still Open In RPF7

- The broad `pf7-m2d-cli-parity`, `pf7-analyze-errors-cli-parity`, and `pf7-legacy-dispatch-parity` oracle rows remain manifest-only until their selected CLI subcases are exhausted.
