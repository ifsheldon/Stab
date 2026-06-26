# Rust Stim Drop-In Rewrite Plan

## Status

Created: 2026-06-26

Target: Stim v1.16.0, tag commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

Pinned upstream source: `vendor/stim` Git submodule at `https://github.com/quantumlib/Stim`, checked out at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

Goal: build Stab as a separate Rust implementation that can become a drop-in replacement for Stim's CLI and core library behavior, with Python bindings added after the Rust and CLI surfaces are mature.

## Guiding Decisions

- Target Stim v1.16.0 as the frozen compatibility baseline.
- Prioritize semantic and statistical equivalence instead of matching Stim's exact random streams.
- Use Rust Nightly and `std::simd` through the `portable_simd` feature as the first-class SIMD abstraction.
- Prefer portable SIMD kernels over architecture-specific AVX, SSE, or NEON implementations during the initial rewrite.
- Keep GPU acceleration out of the first implementation, but preserve backend boundaries that make a later GPU sampler possible.
- Treat `.stim`, `.dem`, and result file formats as public compatibility surfaces, not internal implementation details.
- Build the Rust CLI before Python bindings, because CLI parity is easier to test and makes the oracle harness useful early.

## Architecture

The repository should become a Cargo workspace with separate crates for core logic, CLI behavior, compatibility testing, and benchmarks.

Recommended workspace layout:

| Crate | Purpose |
| --- | --- |
| `stab-core` | Circuit, detector error model, simulator, parser, printer, bit kernels, and public Rust API. |
| `stab-cli` | `stim`-compatible command line binary backed by `stab-core`. |
| `stab-oracle` | Differential test harness that runs Rust Stab and C++ Stim v1.16.0 on the same inputs. |
| `stab-bench` | Criterion and CLI benchmark workloads. |
| `ops` | Rust operational binaries for complex repository automation, compatibility workflows, reports, and release checks. |

Pin `rust-toolchain.toml` to a specific Nightly and enable `#![feature(portable_simd)]` only inside the SIMD kernel modules. Keep non-kernel modules free of nightly-only APIs where possible.

Operational commands should use a root `justfile` with modular files under `justfiles/`.
Recipes should be thin dispatchers for common workflows such as Rust checks, oracle runs, benchmarks, compatibility reports, release checks, and maintenance tasks.
Do not add shell scripts for repository operations.
When operational logic needs branching, validation, structured output, process orchestration, path handling, downloads, or reporting, implement it as an `ops` Rust binary and call it from `just`.

Core public types should be explicit and domain typed: `Circuit`, `CircuitInstruction`, `RepeatBlock`, `Gate`, `Target`, `DetectorErrorModel`, `DemInstruction`, `PauliString`, `CliffordString`, `Tableau`, `CompiledSampler`, and `CompiledDemSampler`.

Use newtypes for stable identifiers and semantic indices: `QubitId`, `DetectorId`, `ObservableId`, `MeasurementIndex`, `MeasureRecordOffset`, `RepeatCount`, and `Probability`.

Define the first SIMD backend around a portable bit block such as `Simd<u64, 4>`, representing 256 logical bits per block. All high-throughput bit operations should route through `stab_core::bits`, so lane width and implementation details can be benchmarked without rewriting algorithms.

## Milestones

Milestones M1 through M3 are intentionally implementation-light. They establish the compatibility matrix, red or manifest-only parity tests, and benchmark baselines before feature implementation begins in M4.

### M0: Project Foundation And Oracle Lab

Theme: infrastructure, compatibility, and repeatability.

Deliverables:

- Convert the repo into the workspace layout described above.
- Add `rust-toolchain.toml` pinned to Nightly and document why Nightly is required.
- Add a root `justfile` and modular files under `justfiles/` for workflow dispatch.
- Add an `ops` crate for complex operational logic.
- Add a staged-aware Rust pre-commit ops binary with `just maintenance::setup-hooks` and `just maintenance::pre-commit`.
- Add `just oracle::fetch` to initialize and validate the `vendor/stim` submodule at Stim v1.16.0 through an `ops` binary.
- Add `just oracle::run` to execute Rust and C++ Stim on the same input and compare results through an `ops` binary.
- Add CI jobs for formatting, linting, tests, oracle smoke tests, and benchmark smoke tests.

Exit criteria:

- `cargo test --workspace` passes.
- `just oracle::version` reports Stim v1.16.0.
- A smoke oracle test compares `stim --help` and a tiny circuit parse or sample flow.

### M1: Feature Parity Inventory And Acceptance Contracts

Theme: define the compatibility target before implementing features.

Deliverables:

- Build a compatibility matrix from Stim v1.16.0 docs, tests, file formats, and CLI command references.
- Use `docs/plans/stim-test-porting-plan.md` as the starting hierarchy for test-file inventory, priority, and milestone ownership.
- Cover planned core surfaces: `.stim`, `.dem`, result formats, gate table, targets, Pauli strings, tableaus, samplers, generated circuits, detector conversion, and detector error model analysis.
- Cover planned CLI surfaces in implementation order: `gen`, `convert`, `sample`, `detect`, `m2d`, `analyze_errors`, and `sample_dem`.
- Classify each behavior as exact-output parity, structural parity, statistical parity, performance parity, explicitly deferred, or intentionally out of scope.
- Define acceptance checks for each future implementation milestone before that milestone starts.

Exit criteria:

- The compatibility matrix identifies every planned feature area and its parity class.
- Deferred surfaces such as `diagram`, `explain_errors`, `repl`, Python bindings, JS/WASM, Crumble, and GPU work are explicitly marked.
- Every implementation milestone below has named parity tests or benchmark checks waiting for it.

### M2: Oracle Corpus And Red Parity Tests

Theme: write feature-equivalence tests before implementation.

Deliverables:

- Create an oracle fixture corpus from `vendor/stim` tests, docs, examples, and generated v1.16.0 outputs.
- Prioritize the P0 and P1 groups from `docs/plans/stim-test-porting-plan.md` before mining deferred ecosystem tests.
- Add oracle test scaffolding that can run C++ Stim v1.16.0 and Rust Stab against the same input once each Rust feature exists.
- Add golden expected outputs for exact-output surfaces such as parser/printer round trips, deterministic `gen`, deterministic sampling, deterministic `detect` and `m2d`, `.dem` printing, and CLI help where compatibility is required.
- Add structural comparators for cases where byte-for-byte output is too strict but semantic equality is required.
- Add statistical test definitions for noisy sampling and DEM sampling.
- Mark not-yet-implemented Stab cases as expected-failing, ignored, or manifest-only so the test suite documents future work without blocking M0.

Exit criteria:

- `just oracle::list` or the equivalent ops command lists the planned corpus by feature area and parity class.
- Exact-output oracle cases can be recorded from the pinned `vendor/stim` submodule.
- Statistical and structural comparators are specified before the sampler and analyzer implementations begin.

### M3: Benchmark Baseline And Performance Contracts

Theme: measure the C++ baseline before optimizing Rust code.

Deliverables:

- Create `stab-bench` and `ops` support for benchmark orchestration before feature implementation.
- Add `just bench::baseline` to run the benchmark matrix against C++ Stim v1.16.0 from `vendor/stim`.
- Record baseline results in a reproducible machine-readable format and a concise human-readable report.
- Define per-feature benchmark contracts for parser/printer throughput, `gen`, tableau operations, sampling analysis time, sampling throughput, `detect`, `m2d`, `analyze_errors`, `.dem` parse/print, and `sample_dem`.
- Separate compile/analysis time from per-shot throughput for sampling benchmarks.

Exit criteria:

- The benchmark harness can run against C++ Stim v1.16.0 before equivalent Rust features exist.
- The primary benchmark circuit matrix is recorded with baseline numbers or documented as pending due to missing local tooling.
- Each later implementation milestone has an associated benchmark target when performance matters.

### M4: Formats, Gate Model, And Canonical Printing

Theme: public data formats before simulation.

Deliverables:

- Implement `.stim` parsing for gates, targets, arguments, tags, comments, whitespace, and `REPEAT` blocks.
- Implement canonical `.stim` printing compatible with Stim v1.16.0 for supported constructs.
- Implement the v1.16.0 gate table, aliases, gate categories, arity rules, argument rules, and target validation.
- Implement the initial `Circuit`, `CircuitInstruction`, `RepeatBlock`, `Gate`, and `Target` APIs.

Exit criteria:

- Upstream circuit parser and printer tests are ported or covered by equivalent oracle cases.
- All golden `.stim` examples round-trip through parse and print.
- Invalid input produces clear domain errors, even if exact C++ error text is not yet required.

### M5: Portable SIMD Bit Core

Theme: maintainable high-throughput primitives.

Deliverables:

- Implement `BitBlock`, `BitSlice`, `BitVec`, and `BitMatrix` around portable SIMD.
- Implement XOR, AND, OR, popcount helpers, row swap, masked row operations, range XOR, transposition helpers, and bit-packed load/store.
- Provide a scalar reference implementation used only by tests.
- Add property tests comparing portable SIMD kernels to scalar kernels.

Exit criteria:

- SIMD and scalar implementations agree across randomized inputs and boundary sizes.
- Criterion benchmarks exist for row XOR, matrix transpose, bit-packed copy, and popcount-like workloads.
- No simulator code directly manipulates raw SIMD lanes outside `stab-core` bit modules.

### M6: Stabilizer Algebra

Theme: correctness of the algebraic core.

Deliverables:

- Implement `PauliString`, `CliffordString`, and `Tableau`.
- Implement tableau composition, inversion, gate conjugation, commutation checks, sign handling, and measurement-related primitives.
- Implement single-qubit Clifford gates, two-qubit Clifford gates, swaps, Pauli products, and common derived operations.

Exit criteria:

- Port upstream algebra tests for Pauli strings, Clifford strings, and tableaus.
- Add randomized property tests for inverse, associativity where applicable, commutation, conjugation, and round-tripping through text.
- Compare selected operations against C++ Stim through the oracle.

### M7: Circuit Generation And Early CLI

Theme: deterministic circuit production and useful developer workflows.

Deliverables:

- Implement `stim gen` compatibility for repetition code, rotated surface code, unrotated surface code, and color code tasks supported by v1.16.0.
- Implement CLI argument parsing for the supported `gen` flags.
- Add a minimal `stim convert` path for parse and canonical print workflows.

Exit criteria:

- `stab-cli gen` output matches v1.16.0 for a golden matrix of distances, rounds, tasks, and noise settings.
- `stab-cli convert` can read `.stim` input and emit canonical `.stim` output for supported circuits.
- Generated circuits become shared fixtures for later simulator, detector, and benchmark milestones.

### M8: Circuit Sampling

Theme: Stim's main performance promise.

Deliverables:

- Implement `CompiledSampler` with analysis separated from per-shot sampling.
- Implement noiseless sampling, Pauli noise, depolarizing noise, heralded errors, measurement/reset behavior, feedback where supported by v1.16.0, and repeat handling.
- Implement `stim sample` with core flags and output formats needed by downstream workflows.
- Implement bit-packed measurement output paths from the beginning.

Exit criteria:

- Deterministic circuits match C++ Stim exactly.
- Noisy circuits pass statistical equivalence checks without requiring identical random streams.
- Benchmarks separately report compile time, single-shot latency, and batch throughput for `1`, `1024`, and `1_000_000` shots.

### M9: Detection Event Workflows

Theme: QEC result processing.

Deliverables:

- Implement measurement-to-detection conversion.
- Implement `stim detect` and `stim m2d`.
- Support the result formats needed for CLI parity, including text and bit-packed formats.
- Handle gauge detector semantics consistently with Stim's documented behavior, without requiring identical arbitrary choices.

Exit criteria:

- Deterministic examples match C++ Stim exactly.
- Gauge-detector examples satisfy structural equivalence checks.
- Bit-packed input and output are covered by round-trip tests and oracle tests.

### M10: Detector Error Model Core

Theme: decoder ecosystem compatibility.

Deliverables:

- Implement `.dem` parser and printer.
- Implement `DetectorErrorModel`, `DemInstruction`, DEM targets, repeat blocks, coordinate handling, detector shifts, observables, and probability validation.
- Implement `stim analyze_errors` initially without advanced flags, then add `--decompose_errors`, `--fold_loops`, `--allow_gauge_detectors`, and approximation behavior.

Exit criteria:

- DEM parse and print tests pass against upstream fixtures.
- Standard generated circuits produce DEMs equivalent to v1.16.0.
- `analyze_errors --fold_loops` avoids expanding high-repeat circuits when Stim v1.16.0 would fold them.

### M11: Detector Error Model Sampling

Theme: fast DEM-based sampling.

Deliverables:

- Implement `CompiledDemSampler`.
- Implement `stim sample_dem`.
- Reuse bit-packed result writers and portable SIMD bit operations from earlier milestones.

Exit criteria:

- Deterministic DEMs match C++ Stim exactly.
- Noisy DEMs pass statistical equivalence checks.
- Benchmarks cover sparse, dense, repeated, and high-detector-count DEMs.

### M12: Performance Hardening

Theme: measured optimization while preserving maintainability.

Deliverables:

- Add a benchmark dashboard comparing Stab against Stim v1.16.0.
- Profile the slowest workloads before optimizing.
- Tune portable SIMD lane widths and memory layout behind the bit-kernel abstraction.
- Add allocation tracking for hot paths.

Exit criteria:

- Beta gate: Stab is within 2x C++ Stim on parser throughput, sampling throughput, `m2d`, `detect`, `sample_dem`, and `analyze_errors`.
- 1.0 gate: Stab is within 1.25x C++ Stim or faster on the primary sampling, detection, DEM sampling, and analyzer benchmarks.
- Any remaining performance gaps are documented with profiler evidence and an owner milestone.

### M13: Python, Browser, And GPU Follow-Ups

Theme: ecosystem expansion after CLI and core parity.

Deliverables:

- Add Python bindings with `pyo3` and `maturin` after CLI parity is stable.
- Add JS/WASM only after the Rust API has settled.
- Revisit Crumble compatibility after JS/WASM support exists.
- Run a GPU spike only for `CompiledSampler` and `CompiledDemSampler`, and only if CPU profiles show a large batch-parallel bottleneck.

Exit criteria:

- Python binding plan is written against the stable Rust API, not the other way around.
- GPU spike has a benchmark proving transfer overhead is amortized before any production implementation is accepted.

## CLI Compatibility Order

Implement CLI commands in this order:

1. `stim gen`
2. `stim convert` for parse and canonical print workflows
3. `stim sample`
4. `stim detect`
5. `stim m2d`
6. `stim analyze_errors`
7. `stim sample_dem`

Defer `stim diagram`, `stim explain_errors`, and `stim repl` until after the core workflows are compatible and benchmarked.

## Test Plan

- Follow a TDD workflow for Stim compatibility: each implementation milestone should begin only after its exact-output, structural, statistical, or benchmark acceptance checks have been defined in the compatibility matrix.
- Vendor or adapt upstream Stim v1.16.0 tests under Apache-2.0 and track each group in a compatibility matrix.
- Use oracle tests for all CLI commands that have been implemented.
- Use fuzzing for `.stim` parsing, `.dem` parsing, result format parsing, CLI argument parsing, and parse-print-parse invariants.
- Use property tests for bit kernels, Pauli algebra, tableau algebra, and simulator invariants.
- Use statistical tests for noisy sampling, including binomial confidence checks for simple channels and chi-square checks for multi-outcome channels.
- Use fixed Rust seeds for reproducibility, while explicitly not requiring C++ Stim random-stream compatibility.

## Benchmark Plan

Benchmarks should report both Rust-only timings and comparisons against Stim v1.16.0.
Milestone M3 owns the benchmark harness and pinned C++ baseline before Rust feature work starts; M12 owns later performance hardening after the core compatible surface exists.

Required workloads:

- `.stim` parse and canonical print throughput.
- `stim gen` for repetition code, rotated surface code, unrotated surface code, and color code circuits.
- Tableau and Pauli primitive operations.
- `stim sample` analysis time and per-shot throughput.
- `stim detect` and `stim m2d` on text and bit-packed input.
- `stim analyze_errors --decompose_errors` and `stim analyze_errors --fold_loops`.
- `.dem` parse and print throughput.
- `stim sample_dem` throughput.

Primary benchmark circuit matrix:

| Circuit family | Distances | Rounds | Notes |
| --- | --- | --- | --- |
| Repetition code | `3`, `5`, `9`, `17` | `d`, `10d` | Good for basic detector workflows. |
| Rotated surface code | `3`, `5`, `9`, `17` | `d`, `10d` | Primary QEC workload. |
| Unrotated surface code | `3`, `5`, `9` | `d`, `10d` | Catches layout and detector differences. |
| High-repeat loop circuit | one small body with huge repeat count | large | Validates loop folding and avoids accidental flattening. |

## Portable SIMD Policy

- Use `std::simd` as the default high-throughput abstraction even though it requires Nightly.
- Keep all direct SIMD usage inside `stab-core` bit-kernel modules.
- Maintain scalar reference kernels for correctness testing, not for production performance.
- Do not add architecture-specific intrinsics during the first implementation.
- If portable SIMD blocks a necessary optimization, document the benchmark and revisit architecture-specific code only after M12.

## GPU Policy

GPU work is deferred.

Potential future GPU candidates:

- Large-batch `CompiledSampler` execution.
- Large-batch `CompiledDemSampler` execution.
- Many independent circuits or parameter points processed in parallel.

Poor initial GPU candidates:

- Parser and printer code.
- DEM generation and graphlike decomposition.
- Error explanation.
- Interactive tableau simulation.
- CLI startup-heavy workloads.

The project should first make CPU portable SIMD fast and correct. A GPU backend should only be considered after benchmarks prove that data transfer and launch overhead are amortized by large batches.

## Risks

- `portable_simd` requires Nightly Rust and may change, so the Nightly version must be pinned.
- Exact Stim CLI behavior includes many edge cases; the oracle harness must be built before large implementation work.
- DEM generation is algorithmically subtle and should not be treated as a simple file-format feature.
- Python compatibility may constrain Rust API choices later, so public Rust types should avoid unnecessary lifetimes and generic-heavy shapes that are painful to bind.
- GPU exploration can distract from correctness and CPU performance, so it is explicitly deferred.

## Acceptance Criteria For First Public Alpha

- The CLI supports `gen`, `convert`, and a useful subset of `sample`.
- `.stim` parsing and canonical printing are reliable against the oracle corpus.
- Portable SIMD bit kernels are in place and benchmarked.
- Algebra tests for `PauliString`, `CliffordString`, and `Tableau` pass.
- The project has a visible compatibility matrix showing implemented and missing Stim v1.16.0 behavior.

## Acceptance Criteria For First Public Beta

- The CLI supports `gen`, `convert`, `sample`, `detect`, `m2d`, `analyze_errors`, and `sample_dem`.
- Standard generated QEC workflows pass oracle or statistical equivalence tests.
- Primary benchmarks are within 2x Stim v1.16.0.
- Deferred surfaces are documented clearly: Python bindings, JS/WASM, Crumble, diagrams, `explain_errors`, `repl`, and GPU acceleration.
