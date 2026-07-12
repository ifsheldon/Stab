# Stab

## Development

This workspace is pinned to Rust Nightly `nightly-2026-06-20` in `rust-toolchain.toml`.
Nightly is required by the roadmap because the core bit kernels will use `std::simd` through `portable_simd`; until those kernels exist, the pin keeps local and CI behavior aligned.

Install the local staged-aware Git hook with:

```sh
just maintenance::setup-hooks
```

Run the same checks manually with:

```sh
just maintenance::pre-commit
```

The hook reads staged Git index entries, treats `vendor/stim` as a submodule pointer, runs Rust formatting and Clippy only for staged Rust-affecting changes, scans staged source blobs for oversized files, and checks instruction-document structure when `README.md`, `AGENTS.md`, `CLAUDE.md`, or `.gitmodules` changes.
Every scanned `README.md` needs a colocated `AGENTS.md`, and every effective `AGENTS.md` source needs at least one `CLAUDE.md` symlink pointing to it.

Generate Rust API documentation with:

```sh
just docs::api
```

The generated Rust API reference is written under `target/doc/`, including `target/doc/stab_core/index.html`.
Run the stricter documentation check before changing public Rust APIs:

```sh
just docs::api-check
```

This check runs rustdoc for the workspace with warnings denied.

Validate the pinned Stim oracle with:

```sh
just oracle::version
```

Run the M0 oracle smoke cases with:

```sh
just oracle::run --case smoke/help
just oracle::run --case smoke/tiny-circuit
```

The tiny-circuit smoke case now runs through the public `stab sample` command.
M7 early CLI compatibility includes `stab gen` for `repetition_code memory`, `surface_code rotated_memory_x`, `surface_code rotated_memory_z`, `surface_code unrotated_memory_x`, `surface_code unrotated_memory_z`, and `color_code memory_xyz`.
The supported `gen` flags are `--code`, `--task`, `--distance`, `--rounds`, `--after_clifford_depolarization`, `--after_reset_flip_probability`, `--before_measure_flip_probability`, `--before_round_data_depolarization`, `--out`, and Stim's accepted no-op `--in`; the deprecated `--gen` spelling is accepted for Stim v1.16.0 compatibility.
`stab convert --in_format=stim --out_format=stim` reads from stdin or `--in`, writes to stdout or `--out`, and emits canonical `.stim` text through the M4 parser/printer.
The result-data conversion surface supports `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64` input and output formats with layouts from explicit `--num_measurements`, `--num_detectors`, and `--num_observables` counts, `--dem`, or `--circuit` plus unique `--types` letters from `M`, `D`, and `L`.
`convert` also supports `--obs_out`, `--obs_out_format`, the legacy top-level `--convert` alias, default `--in_format=01`, and 64-record grouping checks for `ptb64` output.
`convert --bits_per_shot ... --out_format=dets` is rejected like Stim because raw bit width does not identify measurement, detector, or observable record types.
`stab help [topic]` and top-level `stab --help [topic]` provide Stab-native structural help for implemented commands, result formats, and gate names without claiming byte-for-byte Stim help text.
M8 sampling compatibility is done for the selected Rust and CLI surface, including deterministic `01`, `b8`, `r8`, `ptb64`, `hits`, and `dets` output, count-determined measurement and reference-sample-tree helpers, basis, pair, and Pauli-product measurements, feedback, entangling Clifford sampling, repeat handling, and seeded supported noise channels. Exact random-stream parity and public interactive simulator products remain deferred.
Compiled samplers expose heralded-noise measurement records. For Stim v1.16.0 CLI compatibility, `stab sample` omits those columns only on the default one-shot tableau path; multi-shot and `--skip_reference_sample` paths emit them.
M9 detection compatibility includes public `stab detect` and `stab m2d` rows for deterministic detector conversion, observable routing, reference-sample subtraction, text and bit-packed detection records, and M9 benchmark compare runners.
M10 DEM compatibility is done for the selected Rust and CLI analyzer surface, including `.dem` parsing and canonical printing, supported noise and measurement analysis, gauge handling, decomposition controls, generated and folded-loop cases, graphlike and hypergraph search, SAT/WCNF output, sparse reverse tracking, and matched-error values. Full ErrorMatcher provenance and `explain_errors` remain deferred.
M11/M12 DEM sampling compatibility includes deterministic and statistical sampling, detector and observable streams, sampled-error output and replay, supported result formats, bounded parsing, and streaming CLI writers backed by reusable visitor APIs. Materialized APIs and inherently expanded sampled-error records retain documented caps; exact Python API shape remains deferred.

Inspect and check the M2 fixture corpus with:

```sh
just oracle::list
just oracle::list --milestone M4
just oracle::blockers
just oracle::blockers --list
just oracle::blockers --check-selectors
just oracle::record --check-clean
just oracle::run --implemented-only
just oracle::run --milestone M4
just oracle::run --all
```

The fixture manifest lives at `oracle/fixtures/manifest.csv`.
It records fixture ids, upstream sources, command shapes, parity modes, comparator types, expected statuses, implementation status, statistical plans, and source-license notes.
Manifest validation also requires every planned M4 through M11 P0/P1 C++ source from the compatibility matrix to have an explicit fixture row with the matching milestone and parity mode.
Manifest `argv` tokens may use `{fixture_input:inputs/name.ext}` for validated fixture-relative side inputs and `{fixture_output:expected/name.ext}` for exact-output side files written by Stim and Stab during comparison.
Both placeholder forms reject absolute paths, parent-directory traversal, symlinks in fixture paths, and missing fixture inputs; fixture-output placeholders require committed expected files during normal validation and `just oracle::record --check-clean` unless the row is a statistical fixture with `source=fixture_output`.
Statistical fixture rows default to `source=stdout`; `source=fixture_output` requires exactly one `{fixture_output:...}` placeholder, uses that fixture-relative path as a validated scratch label, exact-compares stdout and stderr against pinned Stim, and applies the row's statistical plan to both pinned Stim and Stab side-output bytes.
During oracle runs, fixture-output placeholders are rewritten to fresh scratch paths under `target/oracle/fixture-outputs`; the scratch parent is rejected if any path component is a symlink, and exact-output row side-output bytes are compared in addition to stdout.
`just oracle::run --milestone Mx` scopes execution to implemented fixture rows for that milestone and reports pending red, ignored, or manifest-only rows in the same milestone.
`just oracle::record --check-clean` checks runnable exact-output rows against pinned Stim; library-only parser/printer rows are run in-process by `stab-oracle` and are skipped by recording because they do not have a Stim CLI command.
`just oracle::blockers` validates the schema-versioned blocker closure ledger against the pinned Stim tag and commit, tracked regular upstream source files, exact test and symbol anchors, planned test-family anchors, reproducible statistical plans, required PFM-B owners, test evidence state, implemented primary and supporting oracle rows, typed oracle and benchmark runners, benchmark comparability classes, resource contracts, resource limits, and the frozen SHA-256 semantic inventory.
`just oracle::blockers --check-selectors` additionally runs allowlisted `cargo test ... -- --list` commands through timed, bounded process capture and rejects claimed existing selectors that match no tests.
The blocker-ledger validator currently requires Unix file-identity support and fails closed on other targets instead of accepting a symlink race.

Validate the M1 compatibility matrix with:

```sh
just oracle::matrix --check
just oracle::matrix --milestone M4
```

The matrix lives at `oracle/compatibility-matrix.csv` and records upstream source paths, owners, milestones, priorities, parity modes, comparators, status, acceptance checks, and deferred future buckets.

The active follow-up plans are [comprehensive correctness qualification](docs/plans/comprehensive-correctness-qualification-plan.md) and [comprehensive Stim performance qualification](docs/plans/comprehensive-stim-performance-qualification-plan.md), with execution rules in [GOAL.md](docs/plans/GOAL.md).
These plans will add case-level correctness and feature-level performance disposition ledgers, independently selectable evidence, full and soak tiers, symmetric process CLI comparisons, faithful pinned-Stim adapter coverage, paired confidence intervals, and memory or scaling checks.
Correctness run or report tiers and performance qualification commands remain planned until CQ1 and PQ1 implement them; the existing oracle and benchmark commands below remain authoritative during migration.

CQ0 inventory discovery is implemented through:

```sh
just qualification::correctness-list
just qualification::correctness-list --feature CQ-RESULT-FORMATS
just qualification::correctness-check
just qualification::correctness-regenerate --check
```

These commands validate or deterministically regenerate `oracle/qualification-manifest.json` from the pinned C++ and Python test tree, default-feature rustdoc JSON, and current implemented oracle rows.
The correctness run and report tiers remain planned until CQ1.

Benchmark contracts live at `benchmarks/manifest.csv`.
Each benchmark row owns its runner, threshold class, and comparability class; `just bench::list` prints all three, and manifest validation requires the comparability field to agree with the source-owned compare-note prefix.
The M3 benchmark workflow validates those contracts, records pinned C++ Stim baseline results, and writes generated reports under `target/benchmarks/`.
Any explicit `--out` path must be repository-relative and under `target/benchmarks/`.
`--only` filters use exact benchmark row ids or milestone names such as `M7` on both baseline and compare commands.

```sh
just bench::list
just bench::smoke
just bench::baseline --stim vendor/stim --primary
just bench::baseline --only m7-convert-01-to-b8 --out target/benchmarks/convert-probe-baseline
just bench::compare --milestone M4
just bench::compare --only m7-convert-01-to-b8 --baseline target/benchmarks/convert-probe-baseline/baseline.json --report target/benchmarks/convert-probe-compare
just bench::compare --profile release --primary --report target/benchmarks/latest --require-beta-gate --require-profiler-notes
just bench::compare-allocations --primary --report target/benchmarks/latest-allocations
```

`just bench::baseline` writes `baseline.json` and `report.md` to `target/benchmarks/baseline/latest` by default.
Use `--primary` when recording the M12 baseline consumed by `just bench::compare --primary`.
Use a small `--target-seconds` value for quick local smoke runs, and increase it when recording durable baseline artifacts.
`just bench::compare` runs the benchmark ops binary with Cargo's release profile, reads `target/benchmarks/baseline/latest/baseline.json` by default, and can write `compare.json` plus `report.md` to a repository-relative directory under `target/benchmarks/`.
Use `--require-beta-gate` for completion-style runs where every selected row must prove a Stab median no slower than 1.25x pinned Stim.
Profiler notes for rows slower than 1.5x pinned Stim live under the report directory's `profiler-notes/` folder and must include `Dominant cost:` and `Next owner action:` lines when `--require-profiler-notes` is used.
Use `--thresholds <path>` once regression thresholds exist to fail selected rows that exceed their configured maximum relative ratio or cannot produce a comparable ratio.
`just bench::primary-regression` checks the source-owned M12 threshold file with a Stab-side warmup pass and three recorded measurement runs; the scheduled M12 benchmark workflow runs it against a fresh primary pinned-Stim baseline and uploads the generated reports.
`just bench::compare-allocations` builds `stab-bench` with the optional `count-allocations` feature and adds Stab-side allocation counts plus resident-memory samples to the compare report; keep timing-gate runs on plain `just bench::compare` so allocation instrumentation does not affect timing evidence.
Use `--require-memory-gate --memory-baseline <compare.json>` with `just bench::compare-allocations` once the first complete memory report exists to fail rows whose peak live allocation bytes or sampled resident bytes exceed the M12 25 percent regression budget.
