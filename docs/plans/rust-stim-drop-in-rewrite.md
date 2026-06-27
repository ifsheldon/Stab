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

Pin `rust-toolchain.toml` to a specific Nightly, enable the required crate-level `#![feature(portable_simd)]` gate, and keep direct `std::simd` imports and operations inside the SIMD kernel modules. Keep non-kernel modules free of nightly-only APIs where possible.

Operational commands should use a root `justfile` with modular files under `justfiles/`.
Recipes should be thin dispatchers for common workflows such as Rust checks, oracle runs, benchmarks, compatibility reports, release checks, and maintenance tasks.
Do not add shell scripts for repository operations.
When operational logic needs branching, validation, structured output, process orchestration, path handling, downloads, or reporting, implement it as an `ops` Rust binary and call it from `just`.

Core public types should be explicit and domain typed: `Circuit`, `CircuitInstruction`, `RepeatBlock`, `Gate`, `Target`, `DetectorErrorModel`, `DemInstruction`, `PauliString`, `CliffordString`, `Tableau`, `CompiledSampler`, and `CompiledDemSampler`.

Use newtypes for stable identifiers and semantic indices: `QubitId`, `DetectorId`, `ObservableId`, `MeasurementIndex`, `MeasureRecordOffset`, `RepeatCount`, and `Probability`.

Define the first SIMD backend around a portable bit block such as `Simd<u64, 4>`, representing 256 logical bits per block. All high-throughput bit operations should route through `stab_core::bits`, so lane width and implementation details can be benchmarked without rewriting algorithms.

## Milestones

Milestones M1 through M3 are intentionally implementation-light. They establish the compatibility matrix, red or manifest-only parity tests, and benchmark baselines before feature implementation begins in M4.
Every milestone should leave behind runnable commands, source-owned manifests, and documentation that another agent can use without rediscovering the same facts.

### M0: Project Foundation And Oracle Lab

Objective: make the repository reproducible, staged-checkable, and able to call the pinned Stim v1.16.0 oracle before any large feature work starts.

Tasks:

- Convert the repo into the workspace layout described above, with `stab-core` as the first default member.
- Add `rust-toolchain.toml` pinned to Nightly and document why Nightly is required for `portable_simd`.
- Keep the root `justfile` thin and dispatch operational workflows through modular `justfiles/`.
- Keep complex operational logic in Rust binaries, including staged pre-commit checks and oracle orchestration.
- Add `just oracle::fetch` to initialize, update, and validate the `vendor/stim` submodule at tag `v1.16.0`.
- Add `just oracle::version` to print and assert the pinned Stim tag and commit.
- Add `just oracle::run` to invoke pinned C++ Stim and Stab on the same input and compare exact stdout, stderr class, and exit status when the selected comparator requires it.
- Add CI jobs for formatting, linting, unit tests, oracle smoke tests, and benchmark smoke tests.

Linked tests and benchmarks:

- C++ smoke references: `src/stim.test.cc`, `src/stim/main_namespaced.test.cc`, and `src/stim_included_twice.test.cc` from `docs/plans/stim-test-porting-plan.md`.
- Hook checks: `just maintenance::pre-commit`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.
- Oracle smoke: `stim --help` and one tiny `.stim` parse-print or sample command against the pinned submodule.

Done criteria:

- `just maintenance::setup-hooks` installs a working local pre-commit hook without a tracked shell script.
- `just oracle::version` fails unless `vendor/stim` resolves to `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- `just oracle::run --case smoke/help` and `just oracle::run --case smoke/tiny-circuit` pass.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --all --check` pass in CI and locally.

### M1: Feature Parity Inventory And Acceptance Contracts

Objective: create the compatibility contract that later agents implement against instead of guessing from upstream source files.

Tasks:

- Build a machine-readable compatibility matrix from Stim v1.16.0 docs, tests, file formats, CLI command references, and `docs/plans/stim-test-porting-plan.md`.
- Give every row an upstream source path, Stab owner crate, planned milestone, parity mode, comparator type, priority, and status.
- Cover planned core surfaces: `.stim`, `.dem`, result formats, gate table, targets, Pauli strings, tableaus, samplers, generated circuits, detector conversion, and detector error model analysis.
- Cover planned CLI surfaces in implementation order: `gen`, `convert`, `sample`, `detect`, `m2d`, `analyze_errors`, and `sample_dem`.
- Mark deferred surfaces as explicit future work: diagrams, `explain_errors`, `repl`, Python bindings, JS/WASM, Crumble, Cirq, Sinter, StimFlow, ZX, lattice-surgery helpers, QASM, Quirk, and GPU acceleration.
- Define the acceptance check each implementation milestone must satisfy before the milestone begins.

Linked tests and benchmarks:

- Test hierarchy: all P0, P1, P2, P3, Bench, and Skip groups in `docs/plans/stim-test-porting-plan.md`.
- Benchmark hierarchy: `vendor/stim/file_lists/perf_files` as summarized in the Benchmark Source Hierarchy section of `docs/plans/stim-test-porting-plan.md`.

Done criteria:

- `just oracle::matrix --check` verifies that every P0, P1, and Bench upstream file has a matrix row.
- `just oracle::matrix --milestone M4` through `M12` prints non-empty task lists with parity mode and comparator type.
- No implementation milestone below has an unnamed test or benchmark dependency.
- Deferred rows include a reason and a future-plan bucket.

### M2: Oracle Corpus And Red Parity Tests

Objective: create feature-equivalence tests before the implementation exists, so implementation milestones can turn red cases green.

Tasks:

- Create an oracle fixture manifest that names every planned fixture, upstream source, comparator, command shape, expected status, and milestone.
- Import or generate exact-output fixtures for deterministic parser/printer, `gen`, `convert`, deterministic sampling, `detect`, `m2d`, `.dem` parsing/printing, and CLI help cases.
- Define structural comparator contracts for cases where byte-for-byte output is too strict but semantic equality is required.
  Runnable structural comparator implementations are completed in the owning implementation milestone before any matching row is marked `implemented`.
- Define statistical comparator contracts for noisy circuit sampling and DEM sampling, including sample counts, confidence bounds, fixed Rust seeds, and acceptable false-positive rate.
  Runnable statistical comparator implementations are completed in the owning sampling milestone before any matching row is marked `implemented`.
- Mark not-yet-implemented Stab cases as red, ignored, or manifest-only without hiding them from `just oracle::list`.
- Add source-license notes for any copied upstream tests or fixtures.

Linked tests and benchmarks:

- P0 and P1 C++ groups from `docs/plans/stim-test-porting-plan.md`, especially circuit, command, DEM, generator, IO, simulator, stabilizer, and util-top groups.
- P2 Python binding tests only as semantic-mining sources before Python bindings exist.
- No performance benchmarks are required in M2, but oracle fixture manifests should reference benchmark fixtures when a fixture will later be reused by M3 or M12.

Done criteria:

- `just oracle::list` prints every fixture grouped by milestone, parity mode, and status.
- `just oracle::record --check-clean` can record runnable exact-output fixtures from `vendor/stim` without modifying existing committed fixtures.
  Exact-output parser/printer fixtures that exercise library-only behavior without a Stim CLI equivalent are committed as manifest-only golden files and skipped by recording.
- `just oracle::run --implemented-only` passes for implemented smoke cases.
- `just oracle::run --all` reports unimplemented cases as explicit red, ignored, or manifest-only cases, not as missing metadata.

### M3: Benchmark Baseline And Performance Contracts

Objective: measure the pinned C++ baseline and freeze benchmark contracts before Rust implementations start optimizing against vague targets.

Tasks:

- Create `stab-bench` or an equivalent benchmark package plus `ops` support for benchmark orchestration.
- Add `just bench::baseline` to compile and benchmark C++ Stim v1.16.0 from `vendor/stim`.
- Add `just bench::compare` to read a default or explicit C++ Stim baseline report, validate that the selected report targets pinned Stim v1.16.0, run Stab-side comparison runners for supported rows, report pending Stab runners explicitly, reject unmatched milestone filters, and make `--strict` fail when selected rows are still pending, missing from the selected baseline, backed by invalid placeholder baseline rows, or contract-only without a Stab-side measurement.
- Store benchmark results in machine-readable files under a documented generated-artifact directory and summarize them in a concise human-readable report.
- Define benchmark contracts for parser/printer throughput, `gen`, tableau operations, sampling analysis time, sampling throughput, `detect`, `m2d`, `analyze_errors`, `.dem` parse/print, and `sample_dem`.
  Contracts without a direct pinned C++ executable benchmark runner are allowed as explicit contract-only rows, but they must name their upstream source and owning milestone and must become runnable before an implementation milestone claims a performance comparison for that workload.
  An implementation milestone may still report Stab-only timing for a contract-only row when the milestone explicitly accepts that narrower evidence.
- Separate startup time, compile/analysis time, single-shot latency, and batch throughput where those phases are meaningfully different.

Linked tests and benchmarks:

- Benchmark source hierarchy from `docs/plans/stim-test-porting-plan.md`.
- Primary benchmark circuit matrix from the Benchmark Plan section below.
- Perf sources: `src/stim/circuit/circuit.perf.cc`, `src/stim/gates/gates.perf.cc`, `src/stim/io/measure_record_reader.perf.cc`, `src/stim/mem/*.perf.cc`, `src/stim/simulators/*.perf.cc`, `src/stim/stabilizers/*.perf.cc`, `src/stim/util_bot/*.perf.cc`, and `src/stim/util_top/*.perf.cc`.

Done criteria:

- `just bench::baseline --stim vendor/stim` records pinned C++ baseline results for runnable rows with machine metadata, command metadata, and Stim commit metadata, and reports contract-only rows explicitly.
- `just bench::list` prints every planned benchmark with owning milestone and threshold class.
- `just bench::smoke` runs in CI without requiring long benchmark durations.
- Every implementation milestone M4 through M12 has named benchmark targets or explicitly says no benchmark is required.

### M4: Formats, Gate Model, And Canonical Printing

Objective: implement the public `.stim` data model, gate metadata, parser, validator, and canonical printer before any simulator depends on them.

Tasks:

- Implement `Circuit`, `CircuitInstruction`, `RepeatBlock`, `Gate`, `Target`, and typed argument/newtype APIs in `stab-core`.
- Implement `.stim` parsing for gates, targets, arguments, tags, comments, whitespace, `TICK`, annotations, target combiners, measurement-record targets, sweep targets, detector targets, observable targets, and `REPEAT` blocks.
- Implement canonical `.stim` printing compatible with Stim v1.16.0 for supported constructs.
- Implement the v1.16.0 gate table, aliases, categories, inverse metadata, arity rules, argument rules, and target validation.
- Implement clear domain errors for invalid gates, invalid arguments, invalid target separators, invalid repeat counts, and invalid measurement references.
- Add a minimal parse-print-parse invariant suite and fuzz target for `.stim` input.

Linked tests and benchmarks:

- Direct or oracle tests: C++ Circuit Model, Parser, Targets, And Decomposition; Gates And Gate Metadata; Input And Output Formats where result-format parsing touches circuit commands.
- CLI oracle tests: `src/stim/cmd/command_convert.test.cc` for parse/canonical-print behavior.
- Semantic-mining tests: `src/stim/circuit/*_pybind_test.py`, `src/stim/gates/gates_test.py`, and selected parser examples from Cirq tests only when they expose core `.stim` behavior.
- Benchmarks: `.stim` parse throughput, canonical print throughput, and gate lookup.
  Canonical print currently has a contract-only pinned-Stim baseline because Stim v1.16.0 does not provide a direct C++ printer benchmark runner; M4 reports Stab-only printer timing and does not claim a Stab-vs-Stim printer comparison.

Done criteria:

- `just oracle::run --milestone M4` passes for all M4 exact-output and structural cases.
- `cargo test -p stab-core parser` and `cargo test -p stab-core gates` pass.
- Parser fuzz smoke runs in CI or is documented as a local long-running target; the current local target is `just rust::parser-fuzz`.
- `just bench::compare --milestone M4` reports parser throughput and gate lookup against the M3 C++ baseline, plus Stab-only canonical printer throughput against the explicit contract-only printer row, even if performance is not yet gated.

### M5: Portable SIMD Bit Core

Objective: provide maintainable, portable high-throughput bit primitives that simulators can use without touching raw SIMD lanes.

Tasks:

- Pin Nightly in `rust-toolchain.toml`, enable the crate-level `#![feature(portable_simd)]` gate required by Rust, and isolate direct `std::simd` imports and operations in bit-kernel modules.
- Implement `BitBlock`, `BitSlice`, `BitVec`, and `BitMatrix` around portable SIMD with typed dimensions and explicit ownership.
- Implement XOR, AND, OR, row swap, masked row operations, range XOR, transposition helpers, bit-packed load/store, and popcount-like helpers.
- Provide scalar reference kernels used by tests and comparator code.
- Add randomized property tests over boundary sizes, empty sizes, unaligned tails, repeated row operations, and scalar-vs-SIMD equivalence.
- Prevent simulator modules from depending directly on `std::simd` or architecture-specific intrinsics.

Linked tests and benchmarks:

- Direct tests: C++ Memory And Portable SIMD group from `docs/plans/stim-test-porting-plan.md`.
- Adapted tests: skip C++ container-only tests such as `fixed_cap_vector` and `monotonic_buffer` unless Stab introduces equivalent containers.
- Benchmarks: `src/stim/mem/simd_bit_table.perf.cc`, `src/stim/mem/simd_bits.perf.cc`, `src/stim/mem/simd_word.perf.cc`, and `src/stim/mem/sparse_xor_vec.perf.cc`.

Done criteria:

- `cargo test -p stab-core bits` passes scalar-vs-SIMD property tests.
- `rg "std::simd|portable_simd" crates/stab-core/src` shows the crate-level `portable_simd` feature gate and direct `std::simd` usage only inside approved bit-kernel modules.
- `just bench::compare --milestone M5` reports normalized Stab rates and pinned Stim timings for row XOR, matrix transpose, bit-vector XOR and nonzero checks, sparse table row XOR, sparse item XOR, popcount-like workloads, and Stab-only M5 contract extras such as masked XOR, range XOR, and bit-packed copy.
- M5 benchmark output must label non-comparable contract-smoke workloads; exact optimized 10k bit-table transpose parity is deferred to M12 performance hardening.
- Any required architecture-specific fallback is documented as deferred and not implemented in this milestone.

### M6: Stabilizer Algebra

Objective: implement the algebraic core needed by generation, sampling, tableau simulation, circuit inversion, and detector analysis.

Tasks:

- Implement `PauliString`, `CliffordString`, `Tableau`, and related iterators or views with typed lengths and sign handling.
- Implement tableau composition, inversion, gate conjugation, commutation checks, Pauli products, sign multiplication, random generation hooks, and text round trips.
- Implement single-qubit Clifford gates, two-qubit Clifford gates, swaps, Pauli-product operations, and common derived operations used by Stim tests.
- Implement conversion helpers needed by later `Circuit::to_tableau`, inverse-circuit, flow, and stabilizer-to-tableau operations.
- Add property tests for inverse, identity, associativity where applicable, commutation, conjugation, text round trips, and scalar/reference equivalence.

Linked tests and benchmarks:

- Direct tests: C++ Stabilizers And Algebra group.
- Related util-top tests: `circuit_vs_tableau`, `stabilizers_to_tableau`, `stabilizers_vs_amplitudes`, and `circuit_inverse_unitary` when their dependencies are in scope.
- Semantic-mining tests: Python `pauli_string_pybind`, `clifford_string_pybind`, `tableau_pybind`, `flow_pybind`, and `tableau_simulator_pybind` cases that express core algebra semantics.
- Benchmarks: `src/stim/stabilizers/*.perf.cc` and `src/stim/util_top/stabilizers_to_tableau.perf.cc`.

Done criteria:

- `cargo test -p stab-core stabilizers` passes direct and property tests.
- `just oracle::run --milestone M6` passes selected C++ Stim algebra comparisons.
- `just bench::compare --milestone M6` reports Pauli, Clifford, tableau, tableau-iterator, and stabilizers-to-tableau workloads.
- Public algebra APIs avoid Python-hostile lifetime or generic shapes unless documented.

### M7: Circuit Generation And Early CLI

Objective: ship the first useful `stim`-compatible CLI surface and deterministic generated-circuit fixtures for later simulator, detector, and benchmark work.

Tasks:

- Add `stab-cli` with a binary name or compatibility wrapper that can be invoked as `stab` during development and compared against `stim`.
- Implement `stim gen` compatibility for repetition code, rotated surface code, unrotated surface code, and color code tasks supported by v1.16.0.
- Implement all supported `gen` flags with typed parsing, probability validation, distance/round validation, and helpful errors.
- Implement `stim convert` for `.stim` parse and canonical print workflows.
- Store generated circuit fixture matrices by family, task, distance, rounds, and noise settings for later M8 through M12 reuse.

Linked tests and benchmarks:

- Direct tests: C++ Circuit Generation group and CLI Commands group for `command_gen.test.cc` and `command_convert.test.cc`.
- Related parser tests: M4 `.stim` parse/print fixtures.
- Benchmarks: `stim gen` workloads for repetition code, rotated surface code, unrotated surface code, and color code circuits from the Benchmark Plan, plus `stim convert` CLI startup and canonical `.stim` conversion throughput.

Done criteria:

- `just oracle::run --milestone M7` passes exact-output `gen` and `convert` golden cases.
- `stab-cli gen` output matches Stim v1.16.0 for the compatibility matrix of families, tasks, distances, rounds, and noise settings.
- `stab-cli convert` reads stdin and files and emits canonical `.stim` output for supported circuits.
- `just bench::compare --milestone M7` reports generator throughput for all primary generated-circuit families and convert throughput for supported `.stim` conversion workflows.

### M8: Circuit Sampling

Objective: implement Stim's core circuit-sampling behavior with clear analysis-vs-shot separation and early bit-packed output support.

Tasks:

- Implement `CompiledSampler` with explicit analysis state separated from per-shot sampling.
- Implement noiseless sampling, Pauli noise, depolarizing noise, heralded errors, measurement/reset behavior, feedback supported by v1.16.0, repeat handling, and reference sample behavior.
- Implement `stim sample` with core flags, input/output paths, measurement output formats, bit-packed output, seed handling, and deterministic no-noise behavior.
- Add statistical tests for noisy sampling that do not require C++ random-stream compatibility.
- Add regression tests for result-format padding, endian conventions, text output, and bit-packed output.

Linked tests and benchmarks:

- Direct tests: M8-owned frame and tableau sampling semantics from the C++ Simulators group.
- Deferred simulator tests: detection-output helpers are owned by M9, sparse reverse detector-frame tracking is owned by M10, and graph/vector simulator internals are owned by M12.
- Direct tests: C++ Input And Output Formats group for measurement record formats and sparse shots.
- CLI tests: `src/stim/cmd/command_sample.test.cc`.
- Semantic-mining tests: Python compiled measurement sampler, frame simulator, tableau simulator, and circuit sampling tests.
- Benchmarks: `src/stim/simulators/frame_simulator.perf.cc`, `src/stim/simulators/tableau_simulator.perf.cc`, `src/stim/util_bot/probability_util.perf.cc`, and sampling workloads in the Benchmark Plan.

Done criteria:

- `just oracle::run --milestone M8 --exact` passes deterministic sampling cases.
- `just oracle::run --milestone M8 --statistical` passes noisy sampling statistical cases with documented sample counts and confidence bounds.
- `cargo test -p stab-core sampling` covers repeat blocks, feedback, reset/measurement edge cases, and output-format padding.
- `just bench::compare --milestone M8 --strict` reports compile/analysis time, single-shot latency, batch throughput for `1`, `1024`, and `1_000_000` shots, and representative primary-matrix contract rows with no missing M8 Stab runners, invalid placeholder baselines, empty contract-only placeholders, selected pinned Stim baseline rows, or baseline metadata mismatches.

### M9: Detection Event Workflows

Objective: implement measurement-to-detection conversion and the CLI workflows that decoder pipelines depend on.

Tasks:

- Implement measurement-to-detection conversion from measurement records and circuits with detectors, observables, coordinate shifts, and repeats.
- Implement `stim detect` with supported input/output formats, bit-packed modes, observables, and detector output handling.
- Implement `stim m2d` with measurement input parsing, detector conversion, observable output, and error handling for inconsistent inputs.
- Handle gauge detector semantics structurally, without requiring identical arbitrary choices when Stim documents nondeterminism.
- Add round-trip tests for bit-packed input/output and text input/output across circuit fixtures generated in M7.

Linked tests and benchmarks:

- Direct tests: `src/stim/simulators/measurements_to_detection_events.test.cc`, M9-owned `src/stim/simulators/frame_simulator_util.test.cc` detection-output helpers, and related Python `measurements_to_detection_events_test.py`.
- CLI tests: `src/stim/cmd/command_detect.test.cc` and `src/stim/cmd/command_m2d.test.cc`.
- IO tests: C++ Input And Output Formats group for bit-packed and text result formats.
- Benchmarks: `stim detect` and `stim m2d` on text and bit-packed input from the Benchmark Plan.

Done criteria:

- `just oracle::run --milestone M9 --exact` passes deterministic detection examples.
- `just oracle::run --milestone M9 --structural` passes gauge-detector structural equivalence cases.
- `cargo test -p stab-core detection` covers coordinate shifts, repeats, observables, empty-detector circuits, and invalid measurement references.
- `just bench::compare --milestone M9` reports `detect` and `m2d` throughput separately for text and bit-packed formats.

### M10: Detector Error Model Core

Objective: implement `.dem` compatibility and circuit-to-DEM analysis well enough for decoder ecosystem workflows.

Tasks:

- Implement `.dem` parser and canonical printer.
- Implement `DetectorErrorModel`, `DemInstruction`, DEM targets, repeat blocks, coordinate handling, detector shifts, observables, separators, and probability validation.
- Implement `stim analyze_errors` with a staged flag plan: first default behavior, then `--decompose_errors` with `--block_decompose_from_introducing_remnant_edges` and `--ignore_decomposition_failures`, then `--fold_loops`, `--allow_gauge_detectors`, and approximation behavior.
- Implement loop folding without accidentally flattening high-repeat circuits when Stim would preserve structure.
- Add structural DEM comparators that account for equivalent detector shifts, repeats, and graphlike decomposition where byte-for-byte output is too strict.

Linked tests and benchmarks:

- Direct tests: C++ Detector Error Model group.
- Analyzer tests: `src/stim/simulators/error_analyzer.test.cc`, `src/stim/simulators/error_matcher.test.cc`, `src/stim/simulators/sparse_rev_frame_tracker.test.cc`, and `src/stim/util_top/circuit_to_dem.test.cc`.
- CLI tests: `src/stim/cmd/command_analyze_errors.test.cc`.
- Semantic-mining tests: Python detector error model, DEM instruction, DEM target, matched error, and circuit detector-error-model tests.
- Benchmarks: `src/stim/simulators/error_analyzer.perf.cc`, `.dem` parse/print workloads, and `analyze_errors --decompose_errors` and `--fold_loops` workloads.

Done criteria:

- `just oracle::run --milestone M10 --exact` passes DEM parse/print and simple analyzer cases.
- `just oracle::run --milestone M10 --structural` passes generated QEC circuit DEM equivalence cases.
- `cargo test -p stab-core dem` covers repeat blocks, detector shifts, coordinates, observables, probabilities, separators, and invalid input.
- `just bench::compare --milestone M10` reports `.dem` parse/print and `analyze_errors` workloads with loop-folding cases included.

### M11: Detector Error Model Sampling

Objective: implement fast DEM-based sampling and complete the initial CLI compatibility surface.

Tasks:

- Implement `CompiledDemSampler` with reusable analysis state and per-shot sampling.
- Implement `stim sample_dem` with supported flags, detector output, observable output, bit-packed formats, seed handling, and deterministic behavior where applicable.
- Reuse M5 bit kernels and M8/M9 result writers instead of creating a separate output stack.
- Add exact tests for deterministic DEMs and statistical tests for noisy DEMs.
- Add sparse, dense, repeated, and high-detector-count DEM fixture groups.

M11 accepts a bounded materialized sampler for the initial Rust core and CLI surface.
Completion requires explicit rejection tests for oversized DEM input, bounded DEM line and repeat nesting during parse, bounded repeat expansion, excessive detector or observable output width, excessive sampled-error materialization, replay buffers that would exceed the current materialized limit, truncated `ptb64` replay, and invalid replay shot counts.
The accepted M11 limits are a 64 MiB `sample_dem` DEM input cap, a 1,000,000 line DEM parser cap, a 256 level DEM repeat nesting cap, a 64,000,000 buffered-unit materialization cap, a 64 MiB estimated materialized-buffer byte cap, the existing DEM sampler repeat and expanded-instruction caps, and a 1,048,576 byte cap per requested text replay record.
True streaming output, folded repeat sampling without bounded unrolling, exact output-byte budgeting, and performance thresholds are deferred to M12 performance hardening.

The required M11 `sample_dem` CLI surface is the pinned Stim v1.16.0 flag set `--shots`, `--in`, `--out`, `--out_format`, `--seed`, `--obs_out`, `--obs_out_format`, `--err_out`, `--err_out_format`, `--replay_err_in`, and `--replay_err_in_format`.
Detector output, observable side output, sampled-error output, and replay input must support `01`, `b8`, `r8`, `ptb64`, `hits`, and `dets` where Stim accepts those formats.
M11 acceptance is based on independent detector, observable, and error streams, including replayed error streams.
Stab-only observable routing aliases such as `--append_observables` and hidden `--prepend_observables` are not Stim parity evidence for M11; if retained, they must reject conflicts with each other and with `--obs_out`.

M11 fixture acceptance matrix:

| Fixture group | Required evidence |
| --- | --- |
| Basic deterministic DEM sampling | Exact CLI oracle row and CLI regression test. |
| One-bit noisy DEM sampling | Statistical CLI oracle row with fixed seed, sample count, tolerance, and false-positive budget. |
| Sparse detector ids | Deterministic exact oracle row and bucketed noisy statistical oracle row. |
| Dense detector targets | Deterministic exact oracle row and bucketed noisy statistical oracle row. |
| Repeated detector shifts | Deterministic exact oracle row and bucketed noisy statistical oracle row. |
| High detector ids | Deterministic bit-packed exact oracle row and bucketed noisy statistical oracle row. |
| Observable-only errors | Deterministic side-output exact oracle row and noisy side-output statistical oracle row. |
| Detector-observable correlation and correlated detector combinations | Deterministic exact oracle rows plus bucketed noisy statistical oracle rows where randomness affects the correlation. |
| Observable, error, and replay side streams | Exact side-output oracle rows comparing pinned Stim and Stab side files in addition to stdout. |
| M11-owned simulator internals | Direct Rust structural row for the scoped `dem_sampler` tests. |

Linked tests and benchmarks:

- Direct tests: `src/stim/simulators/dem_sampler.test.cc` and `src/stim/simulators/matched_error.test.cc` where applicable.
- CLI tests: `src/stim/cmd/command_sample_dem.test.cc`.
- Semantic-mining tests: Python DEM sampler and compiled detector sampler tests.
- Benchmarks: `src/stim/simulators/dem_sampler.perf.cc` plus sparse, dense, repeated, and high-detector-count DEM workloads.
- M11 benchmark acceptance is report-only Stab-side throughput from `just bench::compare --milestone M11`; strict pinned-Stim baseline completeness, external CLI-vs-CLI process timing comparability, performance thresholds, and normalized primary-matrix reporting are M12 responsibilities.

Done criteria:

- `just oracle::run --milestone M11 --exact` passes deterministic DEM sampling cases.
- `just oracle::run --milestone M11 --statistical` passes noisy DEM sampling cases with documented confidence bounds.
- `cargo test -p stab-core dem_sampler` covers empty DEMs, observables-only DEMs, repeated DEMs, dense detector cases, and bit-packed output.
- `cargo test -p stab-cli sample_dem` covers the required CLI flag, format, side-output, replay, and resource-limit behavior.
- `just bench::compare --milestone M11` reports sparse, dense, repeated, and high-detector-count DEM sampling throughput.

### M12: Performance Hardening

Objective: close the first public beta performance gate with measured, reviewable optimizations instead of speculative rewrites.

Tasks:

- Freeze the primary benchmark matrix from M3, M7, M8, M9, M10, and M11 so benchmark names and inputs are stable.
- Add `just bench::compare --profile release --report target/benchmarks/latest` to compare Stab against pinned Stim v1.16.0 for the full primary matrix.
- Add a benchmark dashboard or report artifact that records machine metadata, compiler/toolchain metadata, Stim commit, Stab commit, benchmark parameters, median timing, variance, relative ratio, and pass/fail status.
- Profile every benchmark that is slower than the beta gate before optimizing it, and store a short profiler note beside the benchmark report.
- Optimize only behind existing abstractions unless profiler evidence justifies a new abstraction.
- Tune portable SIMD lane widths, memory layouts, allocation patterns, and hot-loop structure behind `stab-core` bit, sampler, detector, and DEM modules.
- Add allocation tracking for parser, sampler compilation, detector conversion, analyzer, and DEM sampler hot paths.
- Add regression thresholds for all workloads that pass the beta gate so future work cannot accidentally erase performance wins.

For M12 benchmark operations, the frozen primary matrix is every benchmark contract row from M4 through M11 except baseline metadata anchors.
The M12 `performance-gate` placeholder row documents the gate and is not itself part of `just bench::compare --primary`.
Completion-style performance runs should pass `--require-beta-gate`, which fails when any selected row lacks a proven Stab-vs-Stim ratio or exceeds the 2.0x beta performance gate.
Profiler notes for compare reports live beside the report under `<report>/profiler-notes/<benchmark-id>.md`.
When `--require-profiler-notes` is passed, every row slower than 1.5x pinned Stim must have a note with non-empty `Dominant cost:` and `Next owner action:` lines.
Regression thresholds use JSON schema version 1 with benchmark ids and `max_relative_ratio` values, and `just bench::compare --thresholds <path>` fails configured selected rows that exceed their threshold or cannot produce a comparable ratio.
Allocation tracking is recorded through `just bench::compare-allocations`, which builds `stab-bench` with the optional `count-allocations` feature and records Stab-side allocation counts and maximum live allocated bytes in `compare.json`.
Timing-gate evidence should use plain `just bench::compare`, because allocation instrumentation changes allocator behavior.

Linked tests and benchmarks:

- Full Benchmark Plan below, including `.stim` parse/print, `gen`, tableau/Pauli primitives, `sample`, `detect`, `m2d`, `analyze_errors`, `.dem` parse/print, and `sample_dem`.
- Benchmark source hierarchy from `docs/plans/stim-test-porting-plan.md`.
- Internal simulator cross-checks for graph and vector simulator behavior from `src/stim/simulators/graph_simulator.test.cc` and `src/stim/simulators/vector_simulator.test.cc`.
- All oracle suites for M4 through M11, because performance changes must preserve functional parity.

Done criteria:

- `just oracle::run --implemented-only` passes before and after performance changes.
- `just bench::compare --profile release --primary` produces a committed or archived report with no missing primary workloads.
- Beta performance gate passes: every primary parser, generator, sampling, detection, DEM parsing, DEM sampling, and analyzer workload is no slower than 2.0x the pinned C++ Stim v1.16.0 median on the same machine.
- Beta memory gate passes: no primary workload regresses peak allocations or resident memory by more than 25 percent relative to the first complete Stab benchmark report unless the report documents an accepted tradeoff.
- Hot-path gate passes: every workload slower than 1.5x Stim has a profiler note naming the dominant cost and the next owner action.
- Regression gate passes: workloads already at or below 1.25x Stim have benchmark thresholds checked by CI smoke or scheduled benchmark automation.
- Any workload that misses the beta gate has a dedicated follow-up issue or milestone entry with profiler evidence, suspected cause, and a proposed implementation path.

## Future Plan

Future work is intentionally outside the M0 through M12 core rewrite sequence. Start any of these only after the Rust library and CLI compatibility surface has passed the first public beta gate or after the roadmap is deliberately revised.

- Python bindings: add `pyo3` and `maturin` bindings after the Rust API is stable enough to avoid binding-driven churn.
- JS/WASM: add browser bindings only after the Rust API has settled and the memory model is compatible with WASM constraints.
- Crumble compatibility: revisit after JS/WASM exists and after Crumble-specific tests are reclassified from P3 to implementation work.
- Cirq, Sinter, StimFlow, ZX, and lattice-surgery integrations: treat as ecosystem projects, not core drop-in CLI requirements.
- Diagrams, `stim explain_errors`, and `stim repl`: plan separately because they need different UX, rendering, and interactivity acceptance checks.
- QASM and Quirk exports: revisit only if drop-in compatibility scope expands beyond the initial CLI order.
- GPU acceleration: run a GPU spike only for `CompiledSampler` and `CompiledDemSampler`, and only if CPU profiles show a large batch-parallel bottleneck.
- GPU acceptance condition: a future GPU implementation must include a benchmark proving that transfer and launch overhead are amortized for the target batch sizes before production code is accepted.

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
