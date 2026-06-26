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
M8 sampling compatibility has started with deterministic `01`, `b8`, `hits`, and `dets` output for the parser-backed Z-basis subset plus seeded `X_ERROR` sampling for the first statistical oracle case; `r8`, `ptb64`, broader noise channels, and reference-sample behavior still belong to later M8 work.

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
```

`just bench::baseline` writes `baseline.json` and `report.md` to `target/benchmarks/baseline/latest` by default.
Use a small `--target-seconds` value for quick local smoke runs, and increase it when recording durable baseline artifacts.
