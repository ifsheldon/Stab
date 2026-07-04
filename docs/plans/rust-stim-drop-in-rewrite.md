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

M0 extracts only oracle-process smoke checks from the upstream smoke references: help-command health, binary namespacing or inclusion health, and one tiny deterministic circuit case.
Full parser behavior, gate metadata, analyzer behavior, and broader CLI mode handling stay with their owning implementation milestones.
Any M0 `sample` path is an oracle-only smoke shim for `smoke-tiny-circuit`; it does not count as implemented `stim sample` CLI compatibility, and M8 remains responsible for the public `sample` command contract.
Before M3 exists, `just bench::smoke` is a compile and wiring smoke for benchmark operations and must not claim benchmark baselines, performance thresholds, or workload parity.
The M3 benchmark package and benchmark matrix replace the M0 smoke-only benchmark check with real baseline and compare commands.

Done criteria:

- `just maintenance::setup-hooks` installs a working local pre-commit hook without a tracked shell script.
- `just oracle::version` fails unless `vendor/stim` resolves to `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- `just oracle::run --case smoke/help` and `just oracle::run --case smoke/tiny-circuit` pass.
- `just bench::smoke` runs as a compile and wiring smoke only; real benchmark rows and baselines remain M3 acceptance.
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
- Manifest-only rows are allowed only when they identify planned subcase groups, fixture families, malformed-input classes, or extraction criteria in their manifest note.
  File-level placeholders must be split or updated with subcase groups before the owning implementation milestone starts.
- Add source-license notes for any copied upstream tests or fixtures.

Linked tests and benchmarks:

- P0 and P1 C++ groups from `docs/plans/stim-test-porting-plan.md`, especially circuit, command, DEM, generator, IO, simulator, stabilizer, and util-top groups.
- P2 Python binding tests only as semantic-mining sources before Python bindings exist.
- No performance benchmarks are required in M2, but oracle fixture manifests should reference benchmark fixtures when a fixture will later be reused by M3 or M12.

Done criteria:

- `just oracle::list` prints every fixture grouped by milestone, parity mode, and status.
- `just oracle::record --check-clean` can record runnable exact-output fixtures from `vendor/stim` without modifying existing committed fixtures.
  Exact-output parser/printer fixtures that exercise library-only behavior without a Stim CLI equivalent are committed as manifest-only golden files and skipped by recording.
- `rg ",manifest-only," oracle/fixtures/manifest.csv` shows every manifest-only row with a note naming planned subcase groups or extraction criteria, not only an upstream filename.
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
- Implement the v1.16.0 gate table, aliases, categories, inverse metadata, arity rules, argument rules, target validation, and bounded Rust metadata accessors.
- Implement clear domain errors for invalid gates, invalid arguments, invalid target separators, invalid repeat counts, and invalid measurement references.
- Add a minimal parse-print-parse invariant suite and fuzz target for `.stim` input.

M4 decomposition scope is structural only.
`coverage-circuit-gate-decomposition` owns target grouping and disjoint segmentation prerequisites used by later decomposition code.
Full semantic `decomposed` behavior for MPP, SPP, pair measurements, base-gate lowering, and tableau or simulator equivalence belongs to the first milestone that implements the required algebra, flow, simulator, or analyzer semantics, such as the M6 util-top rows and later detector/analyzer milestones.
M4 probability-utility scope is closed-unit probability validation and disjoint probability-list validation used by gate argument rules.
Random hit-index sampling and biased random bit generation from `src/stim/util_bot/probability_util.test.cc` are not M4 acceptance criteria; they belong to the first bit or sampler milestone that consumes equivalent APIs and to M12 performance hardening when they become benchmark targets.

Linked tests and benchmarks:

- Direct or oracle tests: C++ Circuit Model, Parser, Targets, And Decomposition; Gates And Gate Metadata; Input And Output Formats where result-format parsing touches circuit commands.
- Structural utility rows: `coverage-circuit-gate-decomposition` and `coverage-util-bot-probability-util`.
- CLI oracle tests: `src/stim/cmd/command_convert.test.cc` for parse/canonical-print behavior.
- Semantic-mining tests: `src/stim/circuit/*_pybind_test.py`, `src/stim/gates/gates_test.py`, and selected parser examples from Cirq tests only when they expose core `.stim` behavior.
- Benchmarks: `.stim` parse throughput, canonical print throughput, and gate lookup.
  Canonical print currently has a contract-only pinned-Stim baseline because Stim v1.16.0 does not provide a direct C++ printer benchmark runner; M4 reports Stab-only printer timing and does not claim a Stab-vs-Stim printer comparison.

Done criteria:

- `just oracle::run --milestone M4` passes for all M4 exact-output and structural cases.
- `cargo test -p stab-core parser` and `cargo test -p stab-core gates` pass.
- `cargo test -p stab-core gate_decomposition` covers M4 structural target grouping and disjoint segmentation without claiming full Stim `decomposed` parity.
- `cargo test -p stab-core probability` covers M4 closed-unit probability validation and disjoint probability-list validation.
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

M6 public Rust API starts with owned `PauliString`, `FlexPauliString`, `CliffordString`, and `Tableau` values.
A public `PauliStringRef` equivalent is not required for M6; borrowed packed views may remain internal unless a later M6 task proves that a public view is necessary for parity or measured performance.
Text parity is split by type: `PauliString` covers real-phase dense syntax and sparse display, while `FlexPauliString` covers phase-general dense syntax with lowercase axis acceptance and sparse indexed syntax.
Python-only phase and binding behavior remains semantic-mining input until the Python API milestone; M6 should not expose Python-hostile borrowed APIs just to mirror pybind-specific behavior early.
M6 random-generation hooks use caller-owned `rand::Rng` values: `PauliString::random`, `PauliString::randomize`, `SingleQubitClifford::random`, `CliffordString::random`, `CliffordString::randomize`, and `Tableau::random`.
Passing a seeded Rust RNG gives reproducible Stab output, but M6 does not require exact Stim C++ random-stream parity.
`PauliString` samples the sign uniformly and each basis independently uniformly over `I`, `X`, `Y`, and `Z`, including sign sampling for zero-length strings.
`CliffordString` samples uniformly over the 24 single-qubit Clifford gates, while `Tableau::random` samples valid Clifford tableaus from a random Clifford-circuit shape instead of promising a uniform Clifford-group distribution.
Exact uniform tableau sampling or random-workload performance parity belongs to M12 if it becomes a primary performance requirement.
M6 owns the algebra-only `unitary_to_tableau` subset from Stim's `stabilizers_vs_amplitudes` tests: square power-of-two matrices are validated as unitary Clifford operations, interpreted with Stim's little-endian or big-endian amplitude order, snapped with Stim's stabilizer-state phase-smoothing threshold, and converted into tableaus up to global phase.
The current M6 fixture covers the upstream known-unitary gate-data loop for 24 canonical single-qubit Clifford matrices and 22 canonical paired-gate matrices, the upstream controlled-gate endian cases for `XCY`, `XCZ`, `ZCX`, and `YCX`, malformed or non-Clifford rejection cases, and Stim-style phase smoothing.
`tableau_to_unitary`, random tableau/unitary roundtrips, and amplitude-simulator cross-checks stay deferred because they require state-vector or matrix-synthesis scope beyond this stabilizer algebra slice.
M6 util-top ownership is limited to deterministic unitary and tableau-backed subsets unless another line below says otherwise.
`coverage-util-top-circuit-flow-generators` owns unitary circuit flow-generator cases and explicit measurement rejection; measurement-record, reset, pair-measurement, and noise-derived flows stay deferred.
`coverage-util-top-has-flow` owns deterministic unsigned unitary-flow checks; sampled checks, measurement-index flows, and observable-rich flows stay deferred.
`coverage-util-top-circuit-inverse-qec` owns the unitary QEC inverse case and explicit measurement-rewrite rejection; reset, measurement, detector, noise, feedback, and detecting-region rewrites stay deferred.
`coverage-util-top-circuit-vs-tableau` owns unitary `Circuit::to_tableau` composition, repeats, annotations, non-unitary ignore or rejection behavior, and basic Stim examples; tableau-to-circuit synthesis stays deferred.
`coverage-util-top-simplified-circuit` owns H/S/CX base decomposition of single-qubit Clifford gates, CZ/CY/SWAP decomposition, recursive repeat simplification, tableau equivalence, and preservation of unsupported gates; all-gate and measurement-rich simplification stay deferred.
`coverage-util-top-mbqc-decomposition` owns the static-table subset for none cases, measurement and reset aliases, H_XY, S, Pauli identity, CX decomposition parsing, and explicit unsupported behavior; full table coverage and sampled-flow verification stay deferred.
`coverage-util-top-stabilizers-to-tableau` owns deterministic stabilizer conversion cases, including redundancy, inconsistent signs, anticommutation, underconstrained systems, inverse-tableau output, signed valid-tableau samples, random-tableau Z-output preservation, and past-iterator-limit conversion.
Implemented util-top rows must keep their manifest notes synchronized with these owned and deferred subcases, and public helper APIs must reject unsupported semantics clearly instead of silently approximating full Stim behavior.

Linked tests and benchmarks:

- Direct tests: C++ Stabilizers And Algebra group.
- Direct Pauli text rows: `coverage-stabilizers-pauli-string`, `coverage-stabilizers-flex-pauli-string`, and `coverage-stabilizers-pauli-string-ref` distinguish real dense text, phase-general dense and sparse text, and the owned-API subset replacing public borrowed views.
- Related util-top tests: `coverage-util-top-circuit-flow-generators`, `coverage-util-top-has-flow`, `coverage-util-top-circuit-inverse-qec`, `coverage-util-top-circuit-inverse-unitary`, `coverage-util-top-circuit-vs-tableau`, `coverage-util-top-simplified-circuit`, `coverage-util-top-mbqc-decomposition`, `coverage-util-top-stabilizers-to-tableau`, and `coverage-util-top-stabilizers-vs-amplitudes`.
- Semantic-mining tests: Python `pauli_string_pybind`, `clifford_string_pybind`, `tableau_pybind`, `flow_pybind`, and `tableau_simulator_pybind` cases that express core algebra semantics.
- Benchmarks: `src/stim/stabilizers/*.perf.cc` and `src/stim/util_top/stabilizers_to_tableau.perf.cc`.

M6 benchmark acceptance is report-only deterministic Stab-side timing from `just bench::compare --milestone M6`.
Pauli, Clifford, and Pauli-iterator rows may be direct operation-shape matches when compare notes say so, but tableau, tableau-iterator, and stabilizers-to-tableau benchmark evidence uses deterministic Stab workloads until M12 decides exact random, fuzz-like, signed-tableau, and 10K-qubit threshold parity.

Done criteria:

- `cargo test -p stab-core stabilizers` passes direct and property tests.
- `cargo test -p stab-core --test stabilizers_vs_amplitudes` passes the M6-owned unitary-to-tableau parity subset.
- `just oracle::run --milestone M6` passes selected C++ Stim algebra comparisons.
- `just oracle::list --milestone M6` shows implemented M6 util-top rows with manifest notes that name owned subcases and deferred subcases.
- `just bench::compare --milestone M6` reports Pauli, Clifford, tableau, tableau-iterator, and stabilizers-to-tableau workloads with normalized rates and compare notes that label direct matches versus report-only deterministic substitutes.
- Public algebra APIs avoid Python-hostile lifetime or generic shapes unless documented.

### M7: Circuit Generation And Early CLI

Objective: ship the first useful `stim`-compatible CLI surface and deterministic generated-circuit fixtures for later simulator, detector, and benchmark work.

Tasks:

- Add `stab-cli` with a binary name or compatibility wrapper that can be invoked as `stab` during development and compared against `stim`.
- Implement `stim gen` compatibility for repetition code, rotated surface code, unrotated surface code, and color code tasks supported by v1.16.0.
- Implement all supported `gen` flags with typed parsing, probability validation, distance/round validation, and helpful errors.
- Implement `stim convert` evidence in two tracks: a Stab-specific `convert --in_format=stim --out_format=stim` canonical parse/print workflow backed by M4 parser/printer behavior, and pinned-Stim-compatible result-data conversion rows backed by `command_convert.test.cc`. The post-beta tight CLI parity slice extends the original staged `01`/`dets` subset to `01`, `b8`, `r8`, `hits`, `dets`, and `ptb64` conversions with explicit counts, `--dem`, `--circuit`, unique `--types`, `--obs_out`, and `--obs_out_format`.
- Store the generated-circuit acceptance matrix in source-owned oracle, direct-test, and benchmark manifests instead of checking in every generated circuit body. M7 exact CLI goldens cover the public command shape for each supported family and task, direct Rust structural tests cover representative larger noisy family/task/distance/round/probability cases, and benchmark rows cover generated-on-demand primary matrix circuits reused by M8 through M12.

Linked tests and benchmarks:

- Direct tests: C++ Circuit Generation group and CLI Commands group for `command_gen.test.cc` and `command_convert.test.cc`.
- Related parser tests: M4 `.stim` parse/print fixtures.
- Benchmarks: `stim gen` workloads for repetition code, rotated surface code, unrotated surface code, and color code circuits from the Benchmark Plan, plus `stim convert` CLI startup, canonical `.stim` conversion throughput, and source-owned result-format convert CLI rows for dense `01`/`b8`, wide packed `b8`, sparse `dets`, `ptb64` input, circuit layout with observable side output, and DEM layout conversion.

Done criteria:

- `just oracle::run --milestone M7` passes exact-output `gen` and `convert` golden cases.
- `stab-cli gen` output matches Stim v1.16.0 for the M7 source-owned acceptance matrix: exact CLI goldens for every supported family and task, Stim-compatible flag-boundary behavior for implemented `gen` arguments, and direct Rust structural tests for representative larger noisy family/task/distance/round/probability cases.
- `stab-cli convert` reads stdin and files, emits canonical `.stim` output for supported circuits when `--in_format=stim --out_format=stim`, and converts implemented result formats with typed layout inference and observable side outputs.
- `just bench::compare --milestone M7` reports Stab-side generator throughput for all primary generated-circuit families and convert throughput for supported `.stim` and result-format conversion workflows. Result-format convert rows use the public `stab convert` CLI path through `stab_cli::run_from`, compare faithful rows against pinned Stim `stim convert`, and keep `01 -> ptb64` as a contract-only row because Stim v1.16.0 rejects that output shape.

### M8: Circuit Sampling

Objective: implement Stim's core circuit-sampling behavior with clear analysis-vs-shot separation and early bit-packed output support.

Tasks:

- Implement `CompiledSampler` with explicit analysis state separated from per-shot sampling.
- Implement noiseless sampling, Pauli noise, depolarizing noise, heralded errors, measurement/reset behavior, feedback supported by v1.16.0, repeat handling, and reference sample behavior.
- Implement `stim sample` with core flags, input/output paths, measurement output formats, bit-packed output, seed handling, and deterministic no-noise behavior.
- Add statistical tests for noisy sampling that do not require C++ random-stream compatibility.
- Add regression tests for result-format padding, endian conventions, text output, and bit-packed output.

For M8, `--skip_loop_folding` is required to be accepted and output-compatible on repeat circuits, but it is not required to force an alternate optimized reference-sample-tree implementation.
Optimized loop-folded reference-sample construction and performance parity stay in M12 unless a later plan amendment promotes them earlier.

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
- `m8-sample-skip-loop-folding` proves `stim sample --skip_loop_folding` repeat-circuit output compatibility, while `coverage-util-top-reference-sample-tree` covers the M8-owned reference-sample-tree subset and records optimized loop-folded construction as deferred.
- `just bench::compare --milestone M8 --strict` reports compile/analysis time, single-shot latency, batch throughput for `1`, `1024`, and `1_000_000` shots, and representative primary-matrix contract rows with no missing M8 Stab runners, invalid placeholder baselines, empty contract-only placeholders, selected pinned Stim baseline rows, or baseline metadata mismatches.

### M9: Detection Event Workflows

Objective: implement measurement-to-detection conversion and the CLI workflows that decoder pipelines depend on.

Tasks:

- Implement measurement-to-detection conversion from measurement records and circuits with detectors, observables, coordinate shifts, and repeats.
- Implement `stim detect` with supported input/output formats, bit-packed modes, observables, and detector output handling.
- Implement `stim m2d` with measurement input parsing, detector conversion, observable output, and error handling for inconsistent inputs.
- Treat `stim detect` Pauli-target `OBSERVABLE_INCLUDE` targets as frame-simulator observable flips for the supported M9 scalar frame subset, while keeping `m2d` measurement-record conversion compatible with pinned Stim by ignoring Pauli targets in conversion plans.
- Handle gauge detector semantics structurally, without requiring identical arbitrary choices when Stim documents nondeterminism.
- Add round-trip tests for bit-packed input/output and text input/output across circuit fixtures generated in M7.

The M9 scalar frame subset for Pauli-target observable detection includes annotations, `R`/`RX`/`RY`, `M`/`MX`/`MY`, `MR`/`MRX`/`MRY`, `MXX`/`MYY`/`MZZ`, `MPP`, `MPAD`, `CX`/`CY`/`CZ` including measurement-record feedback where supported, tableau-backed Clifford gates, single-qubit and two-qubit Pauli noise, depolarizing noise, correlated errors, and heralded single-qubit noise. Unsupported instructions must fail with an explicit sampler-compilation error, and bit-parallel frame storage plus streaming-scale detection conversion remain M12 or later work unless promoted by a future plan amendment.
M9 bit-packed detection parity includes `b8` for `detect` and `m2d` detector and observable streams, plus `ptb64` for `detect` detector output, `detect --obs_out`, and `m2d` measurement input.
`m2d --out_format=ptb64` and `m2d --obs_out_format=ptb64` must reject like pinned Stim v1.16.0 because its detection-event writer does not accept `ptb64` output for `m2d`.
The `ptb64` paths must enforce Stim-compatible 64-shot grouping for generated outputs, read complete measurement-major input groups for `m2d`, and reject zero-width `ptb64` measurement input because the shot count is ambiguous.
M9 now implements sweep input data for public `m2d` detection conversion through `--sweep` and `--sweep_format`, using all-false sweep bits when `--sweep` is omitted. `detect` still rejects sweep-conditioned sampling surfaces until a later sweep-aware detector-sampling milestone introduces typed sweep inputs for sampled detection events.
M9 now implements scoped feedback-removal conversion for `m2d --ran_without_feedback` by applying `circuit_with_inlined_feedback` before compiling the detection converter. This is not full Python `Circuit.with_inlined_feedback` parity; exact loop refolding and broader transform APIs remain future transform work unless a later milestone promotes them explicitly.
M9 accepts a bounded materialized detection-conversion implementation with explicit temporary limits.
The accepted M9 limits are a 1,000,000 bit cap for measurement, detector, and observable record widths, a 64,000,000 buffered-bit cap for materialized measurement samples and detection records, and a 100,000 iteration cap for repeat-block unrolling during conversion planning.
Compiled or streaming detection conversion that processes records in bounded batches, preserves folded repeat structure where possible, avoids duplicate sampler analysis, and removes or justifies these temporary limits is M12 or later work.
M12 implements streaming CLI detection conversion for `detect` and implemented `m2d` formats; the materialized Rust APIs retain the M9 buffered-bit limits for callers that explicitly request in-memory `DetectionConversionOutput`.
The M9 detector utility closure promotes simple detecting regions, basic single-record missing-detector suggestions, and MPP feedback inlining into explicit Rust APIs and executable oracle rows. Broader detecting-region repeat-block traversal, gate and target-shape support, gauge handling, missing-detector row reduction for multi-record detectors, repeated MPP stabilizer-product missing-detector analysis, observable-interaction missing-detector analysis, honeycomb suffix analysis, toric suffix analysis, exact feedback-loop refolding, and full transform API parity remain future detector-analysis or transform work.

Linked tests and benchmarks:

- Direct tests: `src/stim/simulators/measurements_to_detection_events.test.cc`, M9-owned `src/stim/simulators/frame_simulator_util.test.cc` detection-output helpers, `src/stim/simulators/frame_simulator.test.cc` Pauli-target observable cases, and related Python `measurements_to_detection_events_test.py`.
- CLI tests: `src/stim/cmd/command_detect.test.cc` and `src/stim/cmd/command_m2d.test.cc`.
- IO tests: C++ Input And Output Formats group for bit-packed and text result formats.
- Benchmarks: `stim detect` and `stim m2d` on text and bit-packed input from the Benchmark Plan.

M9 benchmark acceptance is report-only Stab-side timing from `just bench::compare --milestone M9`.
Strict pinned-Stim baseline completeness, external CLI-vs-CLI timing comparability, beta-gate ratios, and promoted primary-matrix baseline rows are M12 responsibilities.

Done criteria:

- `just oracle::run --milestone M9 --exact` passes deterministic detection examples.
- `just oracle::run --milestone M9 --structural` passes gauge-detector structural equivalence cases.
- `cargo test -p stab-core detection` covers coordinate shifts, repeats, measurement-record observables, Pauli-target observable flips, empty-detector circuits, invalid measurement references, bounded record-shape validation, sweep-conditioned conversion with all-false defaults and per-shot sweep records, and unsupported sweep-shape rejection.
- `cargo test -p stab-core detecting_regions`, `cargo test -p stab-core missing_detectors`, and `cargo test -p stab-core circuit_with_inlined_feedback` cover the M9 detector utility closure for simple detecting regions, basic missing-detector suggestions, and MPP feedback inlining.
- `cargo test -p stab-core detection_sampling` covers frame-simulator Pauli-target observable parity for basis resets and product measurements.
- `cargo test -p stab-cli m9` covers public `detect` and `m2d` CLI behavior, including `b8`, `detect` `ptb64` outputs, `m2d` `ptb64` input, `m2d` `ptb64` output rejection, `dets`, observable side outputs, route conflicts, zero-shot `detect`, zero-width and oversized `ptb64` input rejection, Pauli-target observable behavior, generated M7 repetition, rotated-surface, unrotated-surface, and color-code `sample -> m2d` round trips against `detect` for `01` and `b8`, sweep-conditioned `m2d --sweep` streaming, all-false omitted-sweep behavior, and scoped `--ran_without_feedback` feedback inlining.
- `just bench::compare --milestone M9` reports `detect`, ordinary `m2d`, sweep-conditioned `m2d`, `m2d --ran_without_feedback`, and report-only detector utility throughput as M9 evidence unless later repeated probe reports promote specific rows into threshold ownership.

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

M10 ErrorMatcher acceptance is a staged direct-Rust subset, not full parity for every upstream provenance case in `src/stim/simulators/error_matcher.test.cc`.
The accepted subset is the implemented `coverage-simulators-error-matcher` row, including small direct circuits, generated repetition-code `DEPOLARIZE1`, generated surface-code `DEPOLARIZE2` filter matching, `PAULI_CHANNEL_2` component matching, DEM filtering, representative reduction, coordinate attribution, and matched-error formatting.
Generated surface-code repeat matching, heralded matching, repeat-contained noise stack frames, and full sparse reverse tracker consumption remain future detector-analysis work until promoted into explicit acceptance rows.
M10 Python semantic-mining rows are direct Rust or CLI semantic rows only; Python binding APIs remain future work.
M10 structural DEM equivalence must normalize detector shifts, repeats, floating probabilities within tolerance, and graphlike target decomposition separators where byte-for-byte DEM output is too strict.
M10 accepts bounded initial resource limits: `analyze_errors` input is capped at 64 MiB, circuit parsing is capped at 1,000,000 lines and 256 nested repeat blocks, non-folded analyzer traversal, ErrorMatcher traversal, and graphlike, hypergraph, and SAT DEM analysis reject flattening plans above 100,000 repeats, 1,000,000 expanded instructions, or 1,000,000 expanded repeat iterations, and ErrorMatcher rejects repeat-contained noise until recursive provenance support is promoted.
Removing or relaxing these limits requires streaming or folded traversal evidence and matching regression tests.
M10 benchmark acceptance is reportable through `just bench::compare --milestone M10`, but any completion report that claims strict Stab-vs-Stim benchmark evidence must cite a fresh selected pinned-Stim baseline path and a matching `just bench::compare --milestone M10 --baseline ... --strict` report.
Baseline completeness and performance thresholds beyond the scoped M10 rows remain M12 performance-hardening responsibilities.

Done criteria:

- `just oracle::run --milestone M10 --exact` passes DEM parse/print and simple analyzer cases.
- `just oracle::run --milestone M10 --structural` passes generated QEC circuit DEM equivalence cases, direct graphlike and hypergraph rows, SAT rows, sparse reverse tracker rows, ErrorMatcher subset rows, matched-error rows, and Python semantic-mining traceability rows.
- `cargo test -p stab-core dem` covers repeat blocks, detector shifts, coordinates, observables, probabilities, separators, invalid input, analytical detector counting through large repeats, and bounded DEM flattening for public analysis APIs.
- `cargo test -p stab-cli m10` covers the staged `analyze_errors` flags plus oversized input and excessive repeat-nesting rejection.
- `just bench::compare --milestone M10` reports `.dem` parse/print and `analyze_errors` workloads with loop-folding cases included.
- Strict M10 benchmark evidence, when claimed in a progress report, is backed by a current `just bench::baseline --only M10 --out ...` artifact and a matching strict compare report that does not rely on stale local `target/` paths.

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
M12 makes `sample_dem` stream detector, observable, sampled-error, and replayed-error outputs by default in the CLI; the materialized Rust APIs retain the M11 buffer-unit and byte caps for callers that request in-memory `DetectionConversionOutput` or sampled-error vectors.

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
- Add `just bench::baseline --primary --out target/benchmarks/baseline/latest` to record a pinned Stim v1.16.0 baseline for the same full primary matrix used by compare.
- Add `just bench::compare --profile release --report target/benchmarks/latest` to compare Stab against pinned Stim v1.16.0 for the full primary matrix.
- Add a benchmark dashboard or report artifact that records machine metadata, compiler/toolchain metadata, Stim commit, Stab commit, benchmark parameters, median timing, variance, relative ratio, and pass/fail status.
- Profile every benchmark that is slower than the beta gate before optimizing it, and store a short profiler note beside the benchmark report.
- Optimize only behind existing abstractions unless profiler evidence justifies a new abstraction.
- Tune portable SIMD lane widths, memory layouts, allocation patterns, and hot-loop structure behind `stab-core` bit, sampler, detector, and DEM modules.
- Harden public CLI resource boundaries introduced by the core milestones: `stab sample`, `stab sample_dem`, `stab detect`, and implemented `stab m2d` paths must be able to write generated output in bounded chunks without materializing all shots, and public circuit or result-input reads must either have a documented cap or a streaming reader.
- Implement compiled or streaming detection conversion for large decoder workloads, processing records in bounded batches, preserving folded repeat structure where possible, avoiding duplicate sampler analysis, and documenting any remaining temporary limits.
- Add allocation tracking for parser, sampler compilation, detector conversion, analyzer, and DEM sampler hot paths.
- Add or promote large generated-code `detect` and `m2d` benchmark rows when streaming detection conversion is implemented, including at least one folded-repeat detector workload and one primary-matrix generated-code workload.
- Add 1.25x regression thresholds for all workloads that meet the regression gate so future work cannot accidentally erase performance wins.

For M12 benchmark operations, the frozen primary matrix is every benchmark contract row from M4 through M11 except baseline metadata anchors and explicit `non-primary-report-only` utility rows.
The M12 `performance-gate` placeholder row documents the gate and is not itself part of `just bench::compare --primary`.
Completion-style performance runs should pass `--require-beta-gate`.
Rows with faithful pinned-Stim baselines must prove a ratio no slower than the active 1.25x beta performance gate.
The completed first beta gate historically used `2.0x`; the active post-beta gate is `1.25x` and is owned by `docs/plans/beta-125-performance-plan.md`.
Compare reports must record a machine-readable comparability class for every row, using the classes `direct-match`, `cli-baseline`, `contract-representative`, `contract-proxy`, `contract-smoke`, `partial-match`, `report-only`, and `contract-only`.
`direct-match` and `cli-baseline` rows are the strongest beta evidence classes because they match either pinned Stim internal operation shape or the same public CLI command and input.
`contract-representative`, `contract-proxy`, `contract-smoke`, `partial-match`, and `report-only` rows may count only as scoped M12 beta evidence when their compare note explains the representative surface, proxy, smoke scope, missing subcases, or non-exact workload.
`contract-only` rows cannot prove a Stab-vs-Stim timing ratio and require source-owned beta waivers while they remain selected by the primary matrix.
When a row has comparable submeasurements, compare reports pair Stim and Stab measurements by normalized name or by direct-match position.
`direct-match` and `cli-baseline` rows use the worse of the row median ratio and the worst paired submeasurement ratio for beta and regression gates; `partial-match` rows with paired evidence use the worst paired ratio so unmatched contract extras remain visible without deciding a Stim-relative gate.
Rows without paired submeasurement evidence keep using the row median ratio.
Completion-style primary beta runs must include `--warmup --measurement-runs 3`, which runs the selected Stab-side workloads once before recording report measurements and then records the median of three Stab-side measurement runs.
The warmup pass is not written into row measurements, but compare reports must record `command.warmup=true` and `command.measurement_runs >= 3`.
Tiny direct-match Stab measurements may batch repeated operations to reduce clock noise, but reported timing must remain normalized to seconds per operation.
Measured `contract-only` rows that cannot have a faithful pinned-Stim ratio may pass the gate only when `--beta-waivers <path>` names a source-owned JSON waiver with a non-empty reason and follow-up; waiver files must reject stale entries, missing baselines, pending runners, invalid baselines, and rows with measured ratios above the beta gate.
Profiler notes for compare reports live beside the report under `<report>/profiler-notes/<benchmark-id>.md`.
When `--require-profiler-notes` is passed, every row slower than 1.5x pinned Stim must have a note with non-empty `Dominant cost:` and `Next owner action:` lines.
M12 profile evidence is final-current for the hot-path gate: every row still slower than 1.5x pinned Stim in the accepted completion compare must have a source-owned profiler note, while rows optimized during M12 must be listed in `benchmarks/profiler-notes/m12/optimization-log.json`.
Optimization-log rows use JSON schema version 2 and must record the benchmark id, before and after compare report paths, source-owned before and after gate statuses, source-owned before and after ratios, hot-path status, a source profiler-note path for any after row still above 1.5x pinned Stim, dominant-cost evidence or a profiler blocker, implementation summary, semantic checks, and follow-up policy.
For new M12 optimization work after this rule, update the source-owned profiler note or optimization log in the same change set as the optimization evidence.
Regression thresholds use JSON schema version 1 or 2 with benchmark ids and `max_relative_ratio` values, and `just bench::compare --thresholds <path>` fails configured rows that are not selected by the compare run, exceed their threshold, or cannot produce a comparable ratio.
Schema version 2 preserves schema-version-1 row thresholds and adds optional exact submeasurement thresholds, allowing stable direct pairs inside mixed rows to be guarded without hiding unstable tiny filters behind row medians.
Measured `contract-only` rows that cannot have a faithful pinned-Stim threshold ratio may pass the timing-regression gate only when `--regression-waivers <path>` names a source-owned JSON waiver with a non-empty reason and follow-up; waiver files must reject stale entries, comparable rows, configured threshold rows, pending rows, and rows with measured ratios.
`just bench::primary-regression` applies the source-owned M12 threshold file and timing-regression waiver file after a Stab-side warmup pass and three recorded measurement runs, and `.github/workflows/m12-benchmarks.yml` runs the same gate on a schedule and on manual dispatch using a fresh primary pinned-Stim baseline.
Allocation and resident-memory tracking are recorded through `just bench::compare-allocations`, which builds `stab-bench` with the optional `count-allocations` feature and records Stab-side allocation counts, maximum live allocated bytes, and sampled resident bytes in `compare.json`.
Completion-style memory runs should pass `--require-memory-gate --memory-baseline <compare.json>` through `just bench::compare-allocations`, which fails selected rows that lack allocation evidence or exceed the first complete Stab allocation report by more than 25 percent.
Schema-version-1 memory baselines keep the historical absolute sampled resident-memory check for compatibility, while schema-version-2 baselines fail rows that lack resident-delta evidence or exceed the first complete Stab resident-delta report by more than 25 percent plus a 64 KiB absolute slack for page-granular RSS sampling noise.
Timing-gate evidence should use plain `just bench::compare`, because allocation instrumentation changes allocator behavior.

Linked tests and benchmarks:

- Full Benchmark Plan below, including `.stim` parse/print, `gen`, tableau/Pauli primitives, `sample`, `detect`, `m2d`, `analyze_errors`, `.dem` parse/print, and `sample_dem`.
- Benchmark source hierarchy from `docs/plans/stim-test-porting-plan.md`.
- Internal simulator cross-checks for graph and vector simulator behavior from `src/stim/simulators/graph_simulator.test.cc` and `src/stim/simulators/vector_simulator.test.cc`.
- CLI resource-boundary regression tests for streaming `sample`, `sample_dem`, `detect`, and implemented `m2d` output plus bounded or streaming public inputs.
- All oracle suites for M4 through M11, because performance changes must preserve functional parity.

Graph/vector simulator evidence is scoped to Stab's public tableau and amplitude semantics for M12; adding public graph or vector simulator APIs remains outside this milestone unless a later milestone makes those APIs part of the drop-in surface.

Done criteria:

- `just oracle::run --implemented-only` passes before and after performance changes.
- Public CLI resource-boundary tests pass: `stab sample`, `stab sample_dem`, `stab detect`, and implemented `stab m2d` paths stream output through the writer, and the implemented public CLI circuit and result-input reads use documented caps or streaming readers.
- `just bench::compare --profile release --primary` produces a committed or archived report with no missing primary workloads.
- Beta performance gate passes: every comparable primary parser, generator, sampling, detection, DEM parsing, DEM sampling, and analyzer workload is assigned a machine-readable comparability class and is no slower than 1.25x the pinned C++ Stim v1.16.0 median on the same machine after a Stab-side warmup pass and three recorded measurement runs, every completion-style beta report records `command.warmup=true` and `command.measurement_runs >= 3`, and every remaining measured `contract-only` primary workload has a checked source-owned waiver explaining why no faithful ratio can be proven before beta.
- Beta memory gate passes: no primary workload regresses peak live allocations by more than 25 percent relative to the first complete Stab benchmark report, and schema-version-2 memory reports do not regress sampled resident deltas beyond the documented resident-delta budget unless the report documents an accepted tradeoff.
- Hot-path gate passes: every workload still slower than 1.5x Stim has a profiler note naming the dominant cost and the next owner action, and every M12-optimized row has an optimization-log entry naming before and after reports, machine-checkable before and after ratios, gate status, hot-path status, dominant-cost evidence, semantic checks, and follow-up policy.
- Regression gate passes: workloads already at or below 1.25x Stim have benchmark thresholds checked by `.github/workflows/m12-benchmarks.yml` scheduled benchmark automation or an equivalent manually dispatched run, and any selected measured `contract-only` primary workload without a faithful threshold ratio has a checked source-owned timing-regression waiver.
- Any workload that misses the beta gate has a dedicated follow-up issue or milestone entry with profiler evidence, suspected cause, and a proposed implementation path.
- Post-beta beta-hardening status: `docs/plans/beta-125-performance-plan.md` owns the stricter `1.25x` beta target; the final clean expanded 85-row evidence was regenerated from Stab commit `c5ccd7967130e764d3319d699ed0a9fe680de81a` with `local_modifications=false`, passes beta with 80 comparable rows and 5 checked no-ratio waivers, passes timing regression with 80 configured threshold rows and 5 checked no-ratio waivers, and passes memory regression for all 85 rows after the byte-aligned `b8 -> b8` convert fast path, exact error-decomposition branch split, streaming parser and exact common plain-instruction fast paths, and schema-version-2 resident-delta memory baseline. `m8-sample-primary-unrotated-surface-contract` remains a beta watch row and needs a dedicated sampler plan if committed-code evidence repeatedly crosses 1.25x.

## Future Plan

Future work is intentionally outside the M0 through M12 core rewrite sequence. Start any of these only after the Rust library and CLI compatibility surface has passed the first public beta gate or after the roadmap is deliberately revised.
Non-deferred partial Rust and CLI surfaces after M12 are governed by [remaining-partial-feature-milestones.md](remaining-partial-feature-milestones.md), [partial-feature-inventory.md](partial-feature-inventory.md), and [GOAL.md](GOAL.md). [partial-feature-closure-plan.md](partial-feature-closure-plan.md) is retained as historical PF planning context. The future list below covers intentionally deferred surfaces and any full-parity work that remains outside those RPF milestones.
Current RPF2 progress implements Rust `Circuit::flattened`, `Circuit::flattened_operations`, and `Circuit::without_noise` with source-owned tests, oracle metadata, and report-only benchmark runners; full `decomposed`, full feedback-inlining transform parity, exact loop refolding, and flow-time-reversal remain governed by the RPF plan.

- Python bindings: add `pyo3` and `maturin` bindings after the Rust API is stable enough to avoid binding-driven churn.
- JS/WASM: add browser bindings only after the Rust API has settled and the memory model is compatible with WASM constraints.
- Crumble compatibility: revisit after JS/WASM exists and after Crumble-specific tests are reclassified from P3 to implementation work.
- Cirq, Sinter, StimFlow, ZX, and lattice-surgery integrations: treat as ecosystem projects, not core drop-in CLI requirements.
- Diagrams, `stim explain_errors`, and `stim repl`: plan separately because they need different UX, rendering, and interactivity acceptance checks.
- QASM and Quirk exports: revisit only if drop-in compatibility scope expands beyond the initial CLI order.
- Full detector-analysis parity: after the PF plan closes selected non-deferred Rust and CLI subcases, promote any remaining broader detecting-region gauge handling, MPP and observable missing-detector analysis, honeycomb suffix analysis, toric suffix analysis, generated surface-code repeat matching, heralded matching, repeat-contained noise stack frames, and full sparse reverse tracker consumption into explicit detector-analysis acceptance rows before claiming full analyzer utility parity.
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
Exclude the deprecated top-level `--detector_hypergraph` alias from Stab CLI parity; users should call `stab analyze_errors` directly.

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
