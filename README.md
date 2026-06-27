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
M8 sampling compatibility has started with deterministic `01`, `b8`, `r8`, `ptb64`, `hits`, and `dets` output for the parser-backed Clifford subset, typed result-format reader/writer helpers, count-determined measurement helpers, basic reference-sample-tree helpers, X/Y/Z-basis measurement and reset, pair and Pauli-product measurement, measurement-record Pauli feedback, Bell-state entangling Clifford sampling, `--skip_reference_sample`/`--frame0`/`--skip_loop_folding`, plus seeded local noise sampling for `X_ERROR`, `Y_ERROR`, `Z_ERROR`, `I_ERROR`, `II_ERROR`, `DEPOLARIZE1`, `DEPOLARIZE2`, `PAULI_CHANNEL_1`, `PAULI_CHANNEL_2`, `HERALDED_ERASE`, and `HERALDED_PAULI_CHANNEL_1`; optimized loop-folded reference-sample-tree construction still belongs to later M8 work.
M9 detection compatibility includes public `stab detect` and `stab m2d` rows for deterministic detector conversion, observable routing, reference-sample subtraction, text and bit-packed detection records, and M9 benchmark compare runners.
M10 DEM compatibility has started with the `.dem` parser/printer, DEM core types, in-process oracle parse-print rows, and a staged `stab analyze_errors` command for deterministic detectors, measurement-flip errors, simple single-qubit Pauli-error analysis, unconditional and conditional correlated Pauli errors, identity-noise no-ops, reset cutoff of pending single-qubit errors, single-qubit `DEPOLARIZE1` analysis, two-qubit `DEPOLARIZE2` analysis, exact-solved and approximate single-qubit `PAULI_CHANNEL_1` analysis with numeric thresholds, approximate two-qubit `PAULI_CHANNEL_2` analysis with numeric thresholds, identical-symptom error merging, and top-level repeat loop folding; graphlike decomposition, general loop folding, gauge-detector analysis, broader approximation behavior, and large analyzer internals remain pending M10 work.
M11 DEM sampling compatibility includes a staged `stab sample_dem` command with deterministic and statistical oracle rows, detector and observable streams, sampled-error output and replay, `ptb64` and `r8` stream coverage, Stim-compatible CRLF text replay handling, zero-shot declared-path validation, a 64 MiB DEM input cap, bounded DEM parser line and nesting limits, bounded materialized output and replay buffers, and report-only DEM sampling benchmark rows; true streaming and strict performance gates remain M12 work.

Inspect and check the M2 fixture corpus with:

```sh
just oracle::list
just oracle::list --milestone M4
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

Validate the M1 compatibility matrix with:

```sh
just oracle::matrix --check
just oracle::matrix --milestone M4
```

The matrix lives at `oracle/compatibility-matrix.csv` and records upstream source paths, owners, milestones, priorities, parity modes, comparators, status, acceptance checks, and deferred future buckets.

Benchmark contracts live at `benchmarks/manifest.csv`.
The M3 benchmark workflow validates those contracts, records pinned C++ Stim baseline results, and writes generated reports under `target/benchmarks/`.
Any explicit `--out` path must be repository-relative and under `target/benchmarks/`.
`--only` filters use exact benchmark row ids or milestone names such as `M7`.

```sh
just bench::list
just bench::smoke
just bench::baseline --stim vendor/stim
just bench::compare --milestone M4
just bench::compare --profile release --primary --report target/benchmarks/latest --require-beta-gate --require-profiler-notes
just bench::compare-allocations --primary --report target/benchmarks/latest-allocations
```

`just bench::baseline` writes `baseline.json` and `report.md` to `target/benchmarks/baseline/latest` by default.
Use a small `--target-seconds` value for quick local smoke runs, and increase it when recording durable baseline artifacts.
`just bench::compare` runs the benchmark ops binary with Cargo's release profile, reads `target/benchmarks/baseline/latest/baseline.json` by default, and can write `compare.json` plus `report.md` to a repository-relative directory under `target/benchmarks/`.
Use `--require-beta-gate` for completion-style runs where every selected row must prove a Stab median no slower than 2.0x pinned Stim.
Profiler notes for rows slower than 1.5x pinned Stim live under the report directory's `profiler-notes/` folder and must include `Dominant cost:` and `Next owner action:` lines when `--require-profiler-notes` is used.
Use `--thresholds <path>` once regression thresholds exist to fail selected rows that exceed their configured maximum relative ratio or cannot produce a comparable ratio.
`just bench::compare-allocations` builds `stab-bench` with the optional `count-allocations` feature and adds Stab-side allocation counts to the compare report; keep timing-gate runs on plain `just bench::compare` so allocation instrumentation does not affect timing evidence.
