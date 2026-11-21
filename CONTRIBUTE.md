# Contributing To Stab

Stab targets semantic compatibility with pinned Stim v1.16.0 while keeping the Rust architecture, tests, and performance evidence small enough to understand. Current work happens directly on `main`; do not create a branch or linked worktree unless the user explicitly changes that policy.

## Setup

Install the Rust toolchain pinned by `rust-toolchain.toml`, CMake with a C++ compiler for the Stim oracle, and `just`. Initialize the submodule and install the staged pre-commit hook:

```text
git submodule update --init --recursive
just maintenance::setup-hooks
just oracle::version
```

`setup-hooks` installs into the repository's resolved hooks directory and refuses to replace a hook that is not a `stab-pre-commit` copy; pass `--force` once to replace a foreign hook or one installed by the earlier copy-based recipe.

## Development Checks

Prefer targeted crate tests during iteration. Before a requested commit, run the checks appropriate to the touched surface and finish with:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked --config "build.warnings='deny'"
cargo test --workspace
just maintenance::pre-commit
```

The Clippy gate denies warnings through Cargo's `build.warnings` setting rather than `-- -D warnings`, so it fails on any rustc, Clippy, or Cargo manifest-lint warning (such as `cargo::unused_dependencies`) while sharing the build cache with plain `cargo clippy` runs.
`just rust::clippy`, the pre-commit hook, and CI run the same command.

Architecture changes also require:

```text
just architecture::check
just architecture::consumer-check
just architecture::docs-check
```

The pre-commit hook is a Rust binary. It checks staged Rust-affecting paths, oversized source blobs, submodule pointers, and the repository instruction-document policy without adding shell launchers.
It reads the index that Git is committing, including the temporary index of `git commit -a` and pathspec commits, and evaluates the oversized-file and instruction-document checks on staged content, never the working tree.
`cargo fmt` and Clippy read the working tree, so the hook fails without running them when a Rust-affecting path has unstaged changes, is missing, or is untracked and not ignored; stage or stash those paths first.
`STAB_PRE_COMMIT_DRY_RUN_TOOLS=1` is a test and diagnostic switch that prints the Rust tool commands instead of running them.

## Stim Compatibility

The atomic source ledger is `oracle/stim-v1.16-parity.toml` plus its named family fragments. [docs/stim-parity.md](docs/stim-parity.md) is generated from it.

```text
just oracle::gates
just oracle::parity-check
just oracle::parity-run
just oracle::parity-render --check
```

On a fresh checkout, run the gate comparison before the parity owners so the pinned Stim library and gate helper are built outside the per-owner execution timeout. Each completed parity family names one meaningful semantic owner. Keep exact output, statistical behavior, CLI behavior, hostile-input handling, and resource contracts in the crate or process boundary that owns them. Round trips are supporting evidence, not an independent compatibility oracle.

Additional pinned checks are:

```text
just oracle::result-formats --check
just oracle::run --all-rows
cargo test -p stab-oracle -- --ignored pinned_stim
```

The result-format corpus, fixture manifest, gate helper, and pinned-Stim probes run the actual pinned Stim source. Every manifest row compares Stab against live pinned Stim or a stored pinned-Stim artifact; rows own their commands, comparators, source attribution, and statistical plans, and they do not promote semantic family status. Comparators are `exact-output`, `help-health`, in-process `structural`, one-sample `statistical` (each side against hand-written analytic probabilities), and `two-sample` (Stim's and Stab's `01` outputs from the same seeded argv tested against each other per column and per declared joint statistic); `just oracle::run --all-rows --statistical` runs both statistical kinds. Pinned Stim prints DEM decimals at `long double` precision, so live DEM text parity is defined on 128-bit `long double` hosts such as the AArch64 baseline, and CI runs these checks on an AArch64 runner.

## Performance

`benchmarks/suite.toml` is the only active performance contract. Its generated matrix is [benchmarks/SUITE.md](benchmarks/SUITE.md).

```text
just bench::e2e-check
just bench::e2e-run --tier smoke --out target/benchmarks/<unique-name>
just bench::e2e-replay --input target/benchmarks/<bundle>
```

Smoke timing is diagnostic. Formal full and soak evidence requires a clean commit, controlled host, release binaries, fixed CPU affinity, temperature below `100 C`, and no swap I/O during measured samples. Every output path must be new. Do not delete samples, waive a failed ratio, relax the `1.25x` Stim parity gate, relax the `1.15x` self-regression gate, or reduce semantic work to obtain a pass.

Formal runs use `--tier full|soak --affinity-cpu <cpu>`. Release authorization is `just bench::e2e-release-check`; it requires the checked AArch64 evidence pair and permits only the documented evidence descendants of its measured source.

Add a persistent benchmark only for a user workflow. Add a diagnostic only after a profile attributes at least 10 percent of such a workflow to the measured component.

## Documentation

[docs/plans/GOAL.md](docs/plans/GOAL.md) is the short active execution contract. [docs/plans/stim-core-parity-and-lean-evidence-plan.md](docs/plans/stim-core-parity-and-lean-evidence-plan.md) owns the current program. Superseded plans and reports remain available through Git history and do not create additional acceptance gates.

Update public documentation in the same change as public behavior, APIs, CLI flags, formats, operational commands, or performance policy. Regenerate derived files instead of editing them by hand.

## Releases

Follow [docs/RELEASING.md](docs/RELEASING.md). Use `just release::publish-order` and `just release::check --out target/releases/<unique-name>` before any irreversible action. Publication and GitHub draft creation remain credential-isolated operations and require the source-current release evidence check.
