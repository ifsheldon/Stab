# Contributing To Stab

Stab targets semantic compatibility with pinned Stim v1.16.0 while keeping the Rust architecture, tests, and performance evidence small enough to understand. Current work happens directly on `main`; do not create a branch or linked worktree unless the user explicitly changes that policy.

## Setup

Install the Rust toolchain pinned by `rust-toolchain.toml`, CMake with a C++ compiler for the Stim oracle, and `just`. Initialize the submodule and install the staged pre-commit hook:

```text
git submodule update --init --recursive
just maintenance::setup-hooks
just oracle::version
```

## Development Checks

Prefer targeted crate tests during iteration. Before a requested commit, run the checks appropriate to the touched surface and finish with:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
just maintenance::pre-commit
```

Architecture changes also require:

```text
just architecture::check
just architecture::consumer-check
just architecture::docs-check
```

The pre-commit hook is a Rust binary. It checks staged Rust-affecting paths, oversized source blobs, submodule pointers, and the repository instruction-document policy without adding shell launchers.

## Stim Compatibility

The atomic source ledger is `oracle/stim-v1.16-parity.toml` plus its named family fragments. [docs/stim-parity.md](docs/stim-parity.md) is generated from it.

```text
just oracle::parity-check
just oracle::parity-run --tier pr
just oracle::parity-run --tier full
just oracle::parity-render --check
```

Each completed parity family names one meaningful semantic owner. Keep exact output, statistical behavior, CLI behavior, hostile-input handling, and resource contracts in the crate or process boundary that owns them. Round trips are supporting evidence, not an independent compatibility oracle.

Additional pinned checks are:

```text
just oracle::result-formats --check
just oracle::gates
just oracle::run --implemented-only
just oracle::matrix --check
```

The result-format corpus and gate helper run the actual pinned Stim source. The fixture corpus and compatibility matrix are supporting coverage maps; they do not own current feature status.

## Performance

`benchmarks/suite.toml` is the only active performance contract. Its generated matrix is [benchmarks/SUITE.md](benchmarks/SUITE.md).

```text
just bench::e2e-check
just bench::e2e-run --tier smoke --out target/benchmarks/<unique-name>
just bench::e2e-replay --input target/benchmarks/<bundle>
```

Smoke timing is diagnostic. Formal full and soak evidence requires a clean commit, controlled host, release binaries, fixed CPU affinity, temperature below `100 C`, and no swap I/O during measured samples. Every output path must be new. Do not delete samples, waive a failed ratio, relax the `1.25x` Stim parity gate, relax the `1.15x` self-regression gate, or reduce semantic work to obtain a pass.

Formal runs use `--tier full|soak --affinity-cpu <cpu>`. Release authorization is `just bench::e2e-release-check`; it remains intentionally unavailable until P9 records the checked AArch64 evidence pointer.

Add a persistent benchmark only for a user workflow. Add a diagnostic only after a profile attributes at least 10 percent of such a workflow to the measured component.

## Documentation

[docs/plans/GOAL.md](docs/plans/GOAL.md) is the short active execution contract. [docs/plans/stim-core-parity-and-lean-evidence-plan.md](docs/plans/stim-core-parity-and-lean-evidence-plan.md) owns the current program. Historical plans and reports preserve context but do not create additional acceptance gates.

Update public documentation in the same change as public behavior, APIs, CLI flags, formats, operational commands, or performance policy. Regenerate derived files instead of editing them by hand.

## Releases

Follow [docs/RELEASING.md](docs/RELEASING.md). Use `just release::publish-order` and `just release::check --out target/releases/<unique-name>` before any irreversible action. Publication and GitHub draft creation remain credential-isolated operations and require the source-current release evidence check.
