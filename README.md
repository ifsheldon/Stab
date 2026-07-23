# Stab

Stab(ilizer) is an agent-native toolkit for quantum error correction (QEC) research: a safe-Rust codebase that researchers and their AI agents can safely modify and extend.

Its first milestone is a drop-in replacement for [Stim](https://github.com/quantumlib/Stim), the standard simulator and analysis tool for QEC research. Selected implemented Stab surfaces have pinned Stim v1.16.0 compatibility evidence; the active qualification state and reopened remediation work are recorded in [docs/plans/GOAL.md](docs/plans/GOAL.md).
Stab currently implements selected `.stim`, `.dem`, `gen`, `convert`, `sample`, `detect`, `m2d`, `analyze_errors`, `sample_dem`, and result-format surfaces. Support and qualification are tracked separately in [docs/stab-feature-checklist.md](docs/stab-feature-checklist.md); a feature being implemented does not by itself mean its compatibility qualification is complete.

> Compatibility: Selected implemented surfaces are checked against the real Stim through pinned parity tests and benchmark comparisons. The pinned Stim v1.16.0 sources are committed in [vendor/stim](vendor/stim). The active goal and feature checklist distinguish implemented, qualified, reopened, and deferred surfaces. Please report discrepancies so they can be reproduced against the pinned target.

The longer-term vision is composable Rust components for QEC tooling.

The exact implemented scope, including deliberate deferrals such as Python bindings, WASM, diagrams, and `explain_errors`, is recorded in [docs/stab-feature-checklist.md](docs/stab-feature-checklist.md).

## Quickstart

Download a prebuilt `stab` binary for Linux AArch64 or macOS AArch64 from the Releases page of this repository, or build from source with the Rust toolchain pinned in `rust-toolchain.toml`:

```sh
cargo install --path crates/stab-cli
```

On macOS, the unsigned binary needs the quarantine attribute removed after download: `xattr -d com.apple.quarantine stab-macos-aarch64`.

Then run a complete QEC workflow against the committed sample data in [examples/](examples/):

```sh
# Generate a distance-3 rotated surface code memory experiment (committed as examples/surface_d3.stim).
stab gen --code surface_code --task rotated_memory_z --distance 3 --rounds 3 --after_clifford_depolarization 0.001 --out surface_d3.stim

# Sample 1000 measurement shots from the circuit.
stab sample --shots 1000 --seed 42 --in surface_d3.stim --out shots.01

# Sample detector events directly, or convert measurement shots into detector events.
stab detect --shots 1000 --seed 42 --in surface_d3.stim --out dets.dets --out_format dets --append_observables
stab m2d --circuit surface_d3.stim --in shots.01 --in_format 01 --out dets_from_measurements.dets --out_format dets --append_observables

# Decompose the circuit into a detector error model (committed as examples/surface_d3.dem) and sample it.
stab analyze_errors --in surface_d3.stim --out model.dem
stab sample_dem --shots 1000 --seed 42 --in model.dem --out dem_dets.dets --out_format dets --append_observables

# Convert result data between formats.
stab convert --in shots.01 --in_format 01 --out shots.b8 --out_format b8 --circuit surface_d3.stim --types M
```

Every command above finishes in well under a second at distance 3.
Larger distances exercise much bigger simulations; Stab enforces documented resource bounds where Stim would attempt unbounded allocation.

Use `stab help commands`, `stab help formats`, and `stab help gates` to explore the supported surface.

## Supported Platforms

Development and all correctness tests and performance benchmarks run on Linux  (AArch64, actually my DGX Spark), and they are currently Linux-only, while macOS (Apple Silicon) is supported but not tested.

The prebuilt Linux and macOS AArch64 binaries are convenience artifacts built by GitHub Actions.

## Built with Codex and Agent Friendly

Stab is an agent-built codebase: the implementation was developed with OpenAI Codex, and the test, oracle, and benchmark qualification harnesses were developed with GPT-5.6 Sol and previously GPT 5.5.

The working style is plan-first and evidence-first, and the workflow artifacts are committed in this repository: milestone plans and qualification contracts in [docs/plans/](docs/plans/), agent operating rules in [AGENTS.md](AGENTS.md), the pinned Stim v1.16.0 reference sources in [vendor/stim](vendor/stim), and the frozen correctness and performance inventories under [oracle/](oracle/) and [benchmarks/](benchmarks/). For our specialized review skills, see [the skill inventory](.agents/skills/).

Key decisions, including the portable-SIMD bit kernels, the streaming CLI architecture, and the machine-checkable qualification program, are documented in the milestone and progress reports under [docs/plans/](docs/plans/).

## Development

Development setup, the staged pre-commit hook, oracle and compatibility-matrix workflows, correctness and performance qualification commands, and benchmark contracts are documented in [CONTRIBUTE.md](CONTRIBUTE.md).
