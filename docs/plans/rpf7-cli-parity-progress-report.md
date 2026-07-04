# RPF7 CLI Parity Progress Report

## Implemented Slice: Public CLI Benchmark And Legacy Dispatch Evidence

Stab now has source-owned report-only benchmark coverage for the PF7 public CLI `m2d --sweep` packed `b8` path, the public CLI `m2d --ran_without_feedback` path, the public CLI `analyze_errors --decompose_errors` path, the public CLI generated-circuit `analyze_errors` path, and accepted legacy `--gen` dispatch.
It also has executable CLI evidence for selected legacy-mode conflicts and explicit `--detector_hypergraph` exclusion.

This slice reuses source-owned M9 and M10 fixtures or generated workloads and routes through `stab_cli::run_from`, so it measures public CLI behavior instead of lower-level conversion helpers.

## Implemented Slice: `m2d` Path IO Evidence

This PFM7 slice promotes source-owned CLI evidence for `stab m2d --circuit`, `--in`, `--out`, `--sweep`, and `--obs_out` path behavior without changing detection-conversion semantics.
The owned positive subcase reads measurements from `--in`, writes detector records to `--out`, writes observable records to `--obs_out`, leaves stdout and stderr empty, and exits successfully.
The owned negative subcases reject a missing `--circuit` path before creating `--out`, reject a missing `--in` path before an unwritable `--out` and before converter setup, reject an unwritable `--out` before a missing `--sweep` and before converter setup, and truncate a writable `--out` before rejecting an unwritable `--obs_out`.
The comparator class is structural CLI behavior against the selected Stim `m2d` command contract: accepted path flags, Stim-style open precedence before converter setup, rejected path errors, stdout behavior, stderr class, and exit status.
No benchmark row changes are needed because this slice tests path-boundary behavior and open precedence rather than a new conversion hot path.

## Implemented Slice: `analyze_errors` Path IO Evidence

This PFM7 slice promotes source-owned CLI evidence for `stab analyze_errors --in` and `--out` behavior without changing analyzer semantics.
The owned positive subcase reads a circuit from `--in`, writes the detector error model to `--out`, leaves stdout and stderr empty, and exits successfully.
The owned negative subcases reject a nonexistent `--in` path before producing stdout, reject an unwritable `--out` path before parsing malformed input, and truncate a writable `--out` path before reporting a parse failure.
The comparator class is structural CLI behavior against the selected Stim `analyze_errors` command contract: accepted path flags, rejected path errors, stdout behavior, stderr class, and exit status.
No benchmark row changes are needed because this slice tests path-boundary behavior rather than a new analyzer hot path.

## Implemented Slice: Accepted Legacy Alias Dispatch Evidence

This PFM7 slice promotes source-owned CLI evidence that the selected legacy top-level aliases dispatch to the same implementation as their canonical subcommands.
The owned positive subcases cover `--gen=repetition_code`, `--convert`, `--sample=2`, `--detect=3`, space-separated `--detect 3`, `--m2d`, and `--analyze_errors`.
Each subcase compares status, stdout bytes, and stderr bytes against the matching canonical `gen`, `convert`, `sample`, `detect`, `m2d`, or `analyze_errors` command.
The comparator class is structural CLI behavior against the selected Stim legacy-dispatch contract: accepted alias spelling, command normalization, stdout behavior, stderr class, and exit status.
No benchmark row changes are needed because the existing PF7 startup row samples the accepted legacy dispatch path through `--gen`; this slice adds correctness evidence for the full selected alias set without adding a new hot path.

## Evidence

Benchmark row:

- `pf7-cli-m2d-sweep-b8` now has a non-primary report-only runner named `stab_pf7_cli_m2d_sweep_b8`, normalized as shots per second.
- `pf7-cli-m2d-feedback-inline` now has a non-primary report-only runner named `stab_pf7_cli_m2d_feedback_inline`, normalized as shots per second.
- `pf7-cli-analyze-errors-decompose` now has a non-primary report-only runner named `stab_pf7_cli_analyze_errors_decompose`, normalized as circuits per second.
- `pf7-cli-analyze-errors-generated` now has a non-primary report-only runner named `stab_pf7_cli_analyze_errors_generated`, normalized as detectors per second on the source-owned d3/r3 rotated-memory-z generated analyzer workload.
- `pf7-cli-legacy-dispatch-startup` now has a non-primary report-only runner named `stab_pf7_cli_legacy_gen_d3_r3`, normalized as dispatches per second.
- Earlier local probe command `just bench::compare --only pf7-cli-m2d-sweep-b8 --only pf7-cli-m2d-feedback-inline --only pf7-cli-analyze-errors-decompose --only pf7-cli-legacy-dispatch-startup --baseline target/benchmarks/pf7-cli-all-probe-baseline/baseline.json --report target/benchmarks/pf7-cli-all-probe-compare` measured `stab_pf7_cli_m2d_sweep_b8=0.000063024s`, or approximately `7.933e4 shots/s`, `stab_pf7_cli_m2d_feedback_inline=0.000071343s`, or approximately `8.410e4 shots/s`, `stab_pf7_cli_analyze_errors_decompose=0.000034352s`, or approximately `2.911e4 circuits/s`, and `stab_pf7_cli_legacy_gen_d3_r3=0.000043936s`, or approximately `2.276e4 dispatches/s`, as report-only evidence on the local machine. The generated analyzer row has runner coverage but no fresh timing probe recorded in this report yet.

Oracle rows:

- `pf7-m2d-path-io-rust` proves `stab m2d --circuit`, `--in`, `--out`, `--sweep`, and `--obs_out` path success, path-error precedence before converter setup, stdout behavior, stderr class, and exit status.
- `pf7-analyze-errors-path-io-rust` proves `stab analyze_errors --in` and `--out` success, missing input path rejection, output-open precedence, stdout behavior, stderr class, and exit status.
- `pf7-legacy-dispatch-accepted-rust` proves selected accepted legacy aliases dispatch to the same command implementation as canonical subcommands for `gen`, `convert`, `sample`, `detect`, `m2d`, and `analyze_errors`.
- `pf7-legacy-dispatch-conflicts-rust` runs selected legacy conflict cases for `--convert`, `--sample`, `--detect`, `--m2d`, `--analyze_errors`, and `--gen=...`, checking nonzero status, empty stdout, and diagnostic stderr.
- `pf7-detector-hypergraph-excluded-rust` proves deprecated `--detector_hypergraph` is not accepted as a mode and is not exposed as a help topic.
- `pf7-legacy-unselected-modes-rust` proves unselected legacy-style `--diagram`, `--explain_errors`, `--repl`, and `--sample_dem` flags fail closed with nonzero status, empty stdout, and diagnostic stderr.

Verification for the `m2d` path-IO slice:

```sh
cargo test -p stab-cli m2d_path_io --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF7 --structural
```

Verification for the `analyze_errors` path-IO slice:

```sh
cargo test -p stab-cli analyze_errors_path --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF7 --structural
```

Verification for the accepted-alias slice:

```sh
cargo test -p stab-cli legacy_dispatch_accepts_selected_aliases --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF7 --structural
```

## Still Open In RPF7

- The broad `pf7-m2d-cli-parity`, `pf7-analyze-errors-cli-parity`, and `pf7-legacy-dispatch-parity` oracle rows remain manifest-only until their selected CLI subcases are exhausted.
