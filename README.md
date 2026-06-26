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

The tiny-circuit smoke case uses a hidden `stab-cli sample` shim that only exists to prove oracle wiring in M0.
It is not `stim sample` compatibility; the real command contract belongs to the M8 sampling milestone.

Compile benchmark targets as a smoke check with:

```sh
just bench::smoke
```
