# Milestone Under-Specification Log

This log records milestone loopholes, ambiguous acceptance criteria, and under-specified scope discovered during milestone implementation or milestone audit.
Use this file for specification gaps only.
Implementation defects, missing tests, benchmark failures, documentation omissions, and code-review findings should be fixed in the milestone work unless a separate follow-up is explicitly accepted.

## Entry Format

```text
## YYYY-MM-DD - Mx: Milestone Title

Status: Open | Resolved | Superseded
Revealed by: implementation, test, benchmark, audit, or review evidence
Current text: the milestone wording that was too weak or ambiguous
Gap: what the milestone failed to specify
Proposed amendment: concrete replacement text or additional done criterion
Resolution: link or note for the plan update that resolved the gap
```

## Resolved Entries

## 2026-06-27 - M5: Memory Test Subcase Granularity

Status: Resolved
Revealed by: full code review of the M5 oracle rows.
Current text: the test-porting plan marked the Memory And Portable SIMD files as P0 for M5 without separating subcases that require APIs not introduced by the M5 portable bit core.
Gap: file-level oracle rows could imply full parity for upstream memory tests that include randomization, shifts, addition, table text parsing, table slicing and resizing, lower-triangular inversion, subset/intersection predicates, and custom allocation/storage utilities.
Proposed amendment: state that M5 owns only the subcases corresponding to the initial Stab bit-core API, and require unsupported upstream subcases to remain deferred until Stab introduces equivalent public or simulator-facing APIs.
Resolution: `docs/plans/stim-test-porting-plan.md` now defines the M5-owned memory subcases, and `oracle/fixtures/manifest.csv` labels implemented M5 memory rows as M5-owned subsets rather than full-file parity.

## 2026-06-27 - M5: Benchmark Compare Semantics

Status: Resolved
Revealed by: milestone audit of the M5 benchmark compare output.
Current text: M5 required `just bench::compare --milestone M5` to report row XOR, matrix transpose, bit-packed copy, sparse XOR, and popcount-like workloads against the M3 baseline.
Gap: the milestone did not distinguish exact upstream workload matches from Stab-only M5 contract-smoke workloads, did not require normalized Stab rates, and did not say whether the current simple matrix transpose helper had to match the upstream 10k optimized transpose benchmark.
Proposed amendment: require M5 compare output to report normalized Stab rates and pinned Stim timings, label non-comparable contract-smoke workloads explicitly, and defer exact optimized 10k bit-table transpose parity to M12 performance hardening.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now names the normalized M5 benchmark evidence and the M12 deferral; `stab-bench compare` prints normalized rates and M5 comparability notes.

## 2026-06-27 - M5: Portable SIMD Feature Gate Location

Status: Resolved
Revealed by: implementation of the M5 portable-SIMD bit kernel.
Current text: M5 said to pin Nightly and isolate `#![feature(portable_simd)]` in bit-kernel modules.
Gap: Rust feature gates are crate-level attributes, so `#![feature(portable_simd)]` cannot be placed only inside a module even when direct `std::simd` imports and operations are module-local.
Proposed amendment: state that the crate-level feature gate is allowed at `stab-core` crate root, while direct `std::simd` imports and operations must stay in approved bit-kernel modules.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now distinguishes the crate-level feature gate from direct SIMD usage.

## 2026-06-27 - M4: Canonical Printer Benchmark Baseline

Status: Resolved
Revealed by: milestone audit of the M4 benchmark evidence.
Current text: M4 required `just bench::compare --milestone M4` to report parser and printer throughput against the M3 C++ baseline, while `m4-circuit-canonical-print` was a contract-only row.
Gap: pinned Stim v1.16.0 has parser and gate lookup perf runners but no direct C++ canonical-printer benchmark runner; using public `stim convert` would benchmark result-format conversion, not `.stim` canonical printing.
Proposed amendment: state that M4 reports parser throughput and gate lookup against the C++ baseline, and reports Stab-only canonical-printer timing against an explicit contract-only printer row without claiming a Stab-vs-Stim printer comparison.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now names the narrower M4 benchmark evidence and the general contract-only benchmark rule.

## 2026-06-27 - M3: Benchmark Compare Acceptance

Status: Resolved
Revealed by: milestone audit of the M3 benchmark harness.
Current text: M3 asks for `just bench::compare` to run Stab and Stim on the same benchmark matrix once Stab supports the feature, but the done criteria only require `bench::baseline`, `bench::list`, and `bench::smoke`.
Gap: the milestone does not define what `bench::compare` must accept, read, report, or fail on before implementation milestones start using it as evidence.
Proposed amendment: require `bench::compare` to read an M3 baseline report or use the documented default, distinguish runnable rows from pending Stab runners, and make `--strict` fail until the owning milestone provides the required Stab runner and complete selected baseline evidence.
Resolution: `stab-bench compare` now reads the default or explicit baseline report, runs Stab comparison runners for supported rows, reports pending rows explicitly, and makes `--strict` fail when any selected row is pending or missing from the selected baseline; `benchmarks/README.md` and `docs/plans/rust-stim-drop-in-rewrite.md` document the behavior.

## 2026-06-27 - M1/M4/M7: CLI Convert Ordering

Status: Resolved
Revealed by: milestone audit of the M1 compatibility matrix and `just oracle::matrix --milestone M4`.
Current text: M1 says planned CLI surfaces are covered in implementation order as `gen`, `convert`, `sample`, `detect`, `m2d`, `analyze_errors`, and `sample_dem`; M4 links `src/stim/cmd/command_convert.test.cc` for parse/canonical-print behavior; M7 tasks say to implement both `stim gen` and `stim convert`.
Gap: the plan does not clearly say whether M4 implements a public `stim convert` subset, only internal parse-print oracle fixtures, or test metadata that M7 later turns into CLI compatibility.
Proposed amendment: state that M4 owns the `.stim` parser/printer library contract and may use `command_convert.test.cc` only as oracle evidence for parse/canonical-print semantics, while M7 owns public `stim convert` CLI compatibility unless the plan explicitly promotes a minimal M4 CLI subset.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now limits M4 benchmarks to parser, printer, and gate lookup, and assigns public `stim convert` CLI compatibility and convert throughput to M7.

## 2026-06-27 - M4/M6/M9: Top-Level Algorithm Fixture Ownership

Status: Resolved
Revealed by: implementation of the M4 oracle rows for circuit, gate, and probability coverage.
Current text: the compatibility matrix and oracle fixture manifest assigned `src/stim/util_top/mbqc_decomposition.test.cc`, `src/stim/util_top/simplified_circuit.test.cc`, and `src/stim/util_top/transform_without_feedback.test.cc` to M4 as `stim-format` rows.
Gap: these upstream tests depend on flow, tableau, simulator, or detector-conversion semantics that M4 does not otherwise own.
Proposed amendment: assign MBQC decomposition and simplified-circuit tests to the tableau milestone and assign transform-without-feedback tests to the detector-conversion milestone.
Resolution: `oracle/compatibility-matrix.csv` and `oracle/fixtures/manifest.csv` now assign MBQC decomposition and simplified-circuit fixtures to M6, and transform-without-feedback fixtures to M9.

## 2026-06-27 - M3: Contract-Only Benchmark Rows

Status: Resolved
Revealed by: implementation of the M3 benchmark manifest.
Current text: M3 requires benchmark contracts for surfaces such as bit-packed `m2d` and `.dem` parse/print while also requiring pinned C++ baseline results.
Gap: some required benchmark contracts do not have a direct `stim_perf` filter or Stim CLI command that exercises the exact future Stab performance surface.
Proposed amendment: allow explicit contract-only benchmark rows when no direct pinned C++ executable baseline exists, require those rows to name their upstream source and owning milestone, and require a runnable benchmark before an implementation milestone claims a Stab-vs-Stim performance comparison.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now makes contract-only benchmark rows explicit in M3.

## 2026-06-27 - M2: Comparator Implementation Ownership

Status: Resolved
Revealed by: milestone audit of the M2 oracle corpus.
Current text: M2 said to define structural and statistical comparators, while later milestones own the first runnable uses of many semantic and statistical comparator families.
Gap: the plan did not say whether M2 must implement every comparator executable or only define comparator contracts and fixture metadata before implementation milestones begin.
Proposed amendment: state that M2 defines comparator contracts and manifest metadata, while the owning M4 through M11 milestones must implement runnable structural or statistical comparator code before marking matching rows `implemented`.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now makes comparator implementation ownership explicit in the M2 task list.

## Open Entries

## 2026-06-27 - M7: Convert Command Circuit Versus Result-Format Scope

Status: Open
Revealed by: implementation and upstream test inspection of `src/stim/cmd/command_convert.test.cc`.
Current text: M7 requires `stim convert` for `.stim` parse and canonical print workflows and links `command_convert.test.cc` as a direct CLI command test source.
Gap: pinned Stim v1.16.0 `command_convert.test.cc` primarily tests measurement, detector, and observable result-format conversion among formats such as `01`, `b8`, `hits`, `r8`, and `dets`, often with `--circuit`, `--dem`, `--types`, and observable-output routing, while `.stim` canonical circuit parse-print behavior is already owned by the M4 core parser/printer fixtures and is not an exact upstream `stim convert` command surface.
Proposed amendment: split M7 convert acceptance into two explicit tracks: a Stab-specific `convert --in_format=stim --out_format=stim` canonical circuit workflow backed by M4 parser/printer tests, and pinned-Stim-compatible result-data conversion rows backed by `command_convert.test.cc`; defer full `b8`, `hits`, `r8`, `--circuit`, `--dem`, `--types`, and `--obs_out` support to the first milestone that owns the corresponding measurement-record and detector-error-model APIs if M7 does not introduce those APIs.
Resolution: pending plan update.

## 2026-06-27 - M6: Random Generation Hook Ownership

Status: Open
Revealed by: milestone audit of the M6 stabilizer algebra implementation and benchmark rows.
Current text: M6 requires random generation hooks and links upstream `tableau_random*`, Clifford random distribution, and stabilizers-to-tableau fuzz and perf coverage.
Gap: the milestone does not define which Rust RNG type, seeding contract, distribution parity, or public random-constructor API must exist before Stab has simulator and sampling consumers.
Proposed amendment: state that M6 must either introduce explicit deterministic random hooks for `CliffordString`, `PauliString`, and `Tableau` with documented seed and distribution contracts, or defer random generation to the first simulator/sampler milestone that consumes those hooks while keeping M6 deterministic algebra and iterator coverage.
Resolution: pending plan update.

## 2026-06-27 - M6: Util-Top Algorithm Subset Boundaries

Status: Open
Revealed by: milestone audit of M6 `circuit_flow_generators`, `has_flow`, `circuit_inverse_qec`, `simplified_circuit`, `mbqc_decomposition`, `circuit_vs_tableau`, and `stabilizers_to_tableau` rows.
Current text: M6 links related util-top tests when their dependencies are in scope, but the oracle manifest records several rows as implemented with notes that defer measurement-rich, detector, noise, sampled-flow, full-gate, tableau-to-circuit, and fuzz variants.
Gap: the milestone does not split deterministic unitary/tableau subset parity from full upstream util-top parity, so an implemented row can be misread as full Stim parity for the entire upstream file.
Proposed amendment: split each related util-top row into explicit subcases owned by M6 and deferred subcases owned by the simulator, detector, or performance-hardening milestones; require public APIs for subset helpers to document unsupported semantics until the deferred rows are implemented.
Resolution: pending plan update.

## 2026-06-27 - M7: Generator Benchmark Comparability

Status: Open
Revealed by: implementation of Stab-side M7 benchmark runners for `just bench::compare --milestone M7 --strict`.
Current text: M7 requires generator throughput for repetition, rotated surface, unrotated surface, and color code circuits, and the benchmark manifest uses pinned Stim CLI rows for `stim gen` plus a `main_sample*` CLI dispatch perf row.
Gap: the plan does not specify whether Stab-side generator benchmark evidence must measure direct Rust generator construction, `stab-cli gen` end-to-end execution, canonical `.stim` printing cost, process startup cost, or all of these separately.
Proposed amendment: split M7 benchmark acceptance into explicit rows for direct Rust generator construction, `stab-cli gen` in-process dispatch, and external process startup or canonical text emission if those are required; keep the current Stab direct generator rows report-only until an exact CLI-vs-CLI threshold is specified.
Resolution: pending plan update.

## 2026-06-27 - M6: Stabilizers Versus Amplitudes Dependency

Status: Open
Revealed by: milestone audit of the M6 linked-test list and compatibility matrix.
Current text: M6 lists `stabilizers_vs_amplitudes` as a related util-top test when dependencies are in scope.
Gap: the plan does not say which amplitude-state or simulator dependency brings this row into scope, and no M6 fixture manifest row currently names the semantic subset that should be proven by the algebra milestone alone.
Proposed amendment: either add a deterministic algebra-only fixture for the subcases that can be checked without an amplitude simulator, or move `stabilizers_vs_amplitudes` to the tableau simulator milestone with a clear dependency note.
Resolution: pending plan update.

## 2026-06-27 - M6: Stabilizer Benchmark Exact Workload Parity

Status: Open
Revealed by: milestone audit of `just bench::compare --milestone M6`.
Current text: M6 requires `just bench::compare --milestone M6` to report Pauli, Clifford, tableau, tableau-iterator, and stabilizers-to-tableau workloads, while benchmark manifest rows point at upstream random, fuzz-like, and large-tableau perf filters.
Gap: the milestone does not distinguish report-only deterministic Stab benchmark runners from exact parity with upstream random and 10K-qubit perf workloads.
Proposed amendment: require M6 compare output to provide deterministic Stab-side timings and normalized rates for each M6 benchmark row, label non-exact benchmark workloads in compare notes, and defer exact random and large-tableau threshold parity to M12 performance hardening after random hooks and optimized tableau internals are specified.
Resolution: pending plan update.

## 2026-06-27 - M6: Stabilizer Algebra Public View And Text Scope

Status: Open
Revealed by: implementation of the first owned Pauli-string algebra slice and upstream stabilizer scan.
Current text: M6 requires `PauliString`, `CliffordString`, `Tableau`, related iterators or views, sign handling, and text round trips.
Gap: the milestone does not say whether Rust must expose a public borrowed `PauliStringRef` equivalent, does not distinguish real-phase C++ `PauliString` text from phase-general `FlexPauliString` sparse and lowercase text, and does not define which Python-facing phase semantics are required before the Python API milestone.
Proposed amendment: state that M6 starts with owned Pauli, FlexPauli, Clifford, and Tableau APIs; borrowed views may stay internal unless a later M6 task proves a public view is necessary; text parity must separately cover real dense `PauliString` syntax and phase-general `FlexPauliString` dense or sparse syntax; Python-only binding behavior is semantic-mining input but not a public API requirement until the Python milestone.
Resolution: pending plan update.

## 2026-06-27 - M4: Gate Decomposition Utility Scope

Status: Open
Revealed by: implementation of `coverage-circuit-gate-decomposition` as a direct Rust oracle row.
Current text: M4 links `src/stim/circuit/gate_decomposition.test.cc` under Circuit Model, Parser, Targets, And Decomposition, but M4's objective is the public `.stim` data model, gate metadata, parser, validator, and canonical printer.
Gap: the upstream file mixes pure circuit-structure helpers, such as target grouping and disjoint segmentation, with semantic MPP/SPP decomposition behavior that later depends on base-gate decomposition, flows, tableaus, and simulator correctness.
Proposed amendment: state that M4 owns structural decomposition prerequisites only, including Pauli-product grouping and disjoint target segmentation; full `decomposed` behavior for MPP, SPP, pair measurements, and base-gate lowering should move to the first milestone that implements the required tableau/simulator semantics or receive its own explicit milestone task.
Resolution: pending plan update.

## 2026-06-27 - M4: Probability Utility Fixture Scope

Status: Open
Revealed by: implementation of `coverage-util-bot-probability-util` as a direct Rust oracle row.
Current text: M4 requires gate argument rules and probability validation, while the test-porting plan points at `src/stim/util_bot/probability_util.test.cc` for probability validation.
Gap: the referenced upstream file also tests `sample_hit_indices` and biased random bit generation, which require RNG and bit-storage behavior that M4 does not otherwise define.
Proposed amendment: state that M4 owns only closed-unit probability validation and disjoint probability-list validation from this file; random hit-index sampling and biased bit generation should move to the first milestone that introduces equivalent RNG and bit/sampler APIs.
Resolution: pending plan update.

## 2026-06-27 - M2: Manifest-Only Subcase Granularity

Status: Open
Revealed by: milestone audit of the M2 manifest coverage rows.
Current text: M2 and the test-porting plan allow red or manifest-only oracle cases for all P0 and P1 files needed by M4 through M11.
Gap: file-level manifest-only rows can satisfy coverage without identifying the upstream subcases, fixture families, malformed-input cases, or extraction criteria that future implementation milestones must port.
Proposed amendment: require manifest-only rows to name planned subcase groups or extraction criteria for each upstream test file before the owning implementation milestone starts.
Resolution: pending plan update.

## 2026-06-26 - M0: Upstream Smoke References Overreach

Status: Open
Revealed by: milestone audit of the M0 oracle lab implementation.
Current text: M0 links `src/stim.test.cc`, `src/stim/main_namespaced.test.cc`, and `src/stim_included_twice.test.cc` as C++ smoke references.
Gap: those upstream files include behavior from later milestones, including circuit parsing, gate metadata, analyzer behavior, and richer CLI mode handling, so treating the full files as M0 requirements would pull M4, M6, and M10 work into the foundation milestone.
Proposed amendment: clarify that M0 extracts only oracle-process smoke checks from these files, specifically help-command health, main binary namespacing health, and one tiny deterministic circuit case; all parser, gate table, analyzer, and broader CLI behavior stays with later milestones.
Resolution: pending plan update.

## 2026-06-26 - M0: Oracle Tiny Sample Shim Boundary

Status: Open
Revealed by: milestone audit and full-code-review of the M0 `stab-cli sample` smoke shim.
Current text: M0 requires `just oracle::run --case smoke/tiny-circuit`, while the CLI compatibility order defers real `sample` support to M8.
Gap: the plan does not say whether a minimal M0 sample command counts as CLI compatibility or is only an oracle fixture target.
Proposed amendment: state that any M0 sample path is an oracle-only smoke shim and does not count as implemented `stim sample` compatibility; M8 remains responsible for the public `sample` command contract.
Resolution: pending plan update.

## 2026-06-26 - M0: Benchmark Smoke Before Benchmark Harness

Status: Open
Revealed by: milestone audit and full-code-review of `just bench::smoke`.
Current text: M0 requires CI benchmark smoke tests, while M3 owns the benchmark package, baseline measurements, benchmark matrix, and performance contracts.
Gap: before M3, benchmark smoke can only prove workspace wiring unless the plan requires an explicit placeholder benchmark target.
Proposed amendment: clarify whether M0 benchmark smoke is compile-only workspace smoke or require a tiny explicit benchmark target that is intentionally replaced by the M3 benchmark harness.
Resolution: pending plan update.
