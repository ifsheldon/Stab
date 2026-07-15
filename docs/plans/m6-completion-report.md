# M6 Completion Report

## Milestone

M6: Stabilizer Algebra.

Objective: implement the algebraic core needed by generation, sampling, tableau simulation, circuit inversion, and detector analysis.

## Status

Complete against the clarified M6 stabilizer-algebra contract.

M6 implements owned Pauli, flexible Pauli, Clifford string, flow, tableau, tableau iterator, circuit-to-tableau, inverse-circuit, simplified-circuit, MBQC decomposition, stabilizers-to-tableau, and unitary-to-tableau subsets needed by the Rust core and later CLI workflows.
Python binding APIs, exact C++ random-stream parity, public borrowed Pauli-string views, full state-vector round trips, full all-gate decomposition, measurement-rich flow semantics, and exact random 10k-qubit performance parity remain deferred by the roadmap.

CQ2 qualification later hardened caller-sized Algebra APIs without expanding the selected product surface. `PauliString`, `FlexPauliString`, `CliffordString`, `Tableau`, `PauliStringIterator`, and `Flow` constructors now return typed resource errors; Clifford growth, aggregate Flow classical terms, dense Tableau and circuit conversion, stabilizer solving, random Tableau construction, unitary conversion, width-weighted compact-repeat Tableau work, and annotation-only identity-flow output have explicit admission contracts documented in the roadmap and checked by `crates/stab-core/tests/cq2_algebra_resources.rs`. `Circuit::to_tableau` also covers Hermitian `SPP` and `SPP_DAG` operations, including repeated-qubit phase reduction, and raises compact repeat bodies by identity-aware binary exponentiation.

## Tests Ported Or Created

- Ported or adapted property tests from `src/stim/stabilizers/clifford_string.test.cc`, `flex_pauli_string.test.cc`, `pauli_string.test.cc`, `pauli_string_iter.test.cc`, `pauli_string_ref.test.cc`, `tableau.test.cc`, and `tableau_iter.test.cc`.
- Ported or adapted structural tests from `src/stim/stabilizers/flow.test.cc`.
- Ported or adapted util-top tests from `src/stim/util_top/circuit_flow_generators.test.cc`, `circuit_inverse_qec.test.cc`, `circuit_inverse_unitary.test.cc`, `circuit_vs_tableau.test.cc`, `has_flow.test.cc`, `mbqc_decomposition.test.cc`, `simplified_circuit.test.cc`, `stabilizers_to_tableau.test.cc`, and `stabilizers_vs_amplitudes.test.cc`.
- Added semantic-mining coverage from Python Pauli, Clifford, tableau, flow, and circuit-flow-generator tests where the behavior belongs to Rust core algebra instead of Python bindings.
- Added property tests for identity, inverse, associativity where applicable, commutation, conjugation, text round trips, deterministic seeded random hooks, and valid tableau invariants.

## Implementation Areas

- `crates/stab-core/src/stabilizers/` owns `PauliString`, `FlexPauliString`, `CliffordString`, `Tableau`, tableau iteration, stabilizer conversion, Pauli multiplication, sign handling, random hooks, and unitary-to-tableau conversion.
- `crates/stab-core/src/flow.rs` and `crates/stab-core/src/circuit_flow.rs` own flow parsing, display, multiplication, circuit generator derivation, and M6 deterministic unitary flow checks.
- `crates/stab-core/src/circuit_tableau.rs`, `circuit_inverse.rs`, `circuit_simplify.rs`, and `mbqc_decomposition.rs` own M6 circuit algebra helpers.
- `crates/stab-core/tests/stabilizers.rs`, `stabilizer_flows.rs`, `circuit_flow_generators.rs`, `circuit_inverse.rs`, `circuit_inverse_qec.rs`, `circuit_tableau.rs`, `circuit_flows.rs`, `mbqc_decomposition.rs`, `circuit_simplify.rs`, `stabilizers_to_tableau.rs`, and `stabilizers_vs_amplitudes.rs` hold the direct M6 test surface.
- `oracle/fixtures/manifest.csv` records the implemented M6 oracle rows and the owned or deferred subcases in each row note.
- `benchmarks/manifest.csv` records six M6 benchmark rows for Clifford strings, Pauli strings, Pauli iterators, tableaus, tableau iterators, and stabilizers-to-tableau conversion.

## Done Criteria

| Requirement | Status | Evidence |
| --- | --- | --- |
| Implement `PauliString`, `CliffordString`, `Tableau`, and related iterators or views with typed lengths and sign handling | Satisfied | `crates/stab-core/src/stabilizers/`; implemented rows `coverage-stabilizers-pauli-string`, `coverage-stabilizers-flex-pauli-string`, `coverage-stabilizers-clifford-string`, `coverage-stabilizers-tableau`, `coverage-stabilizers-pauli-string-iter`, and `coverage-stabilizers-tableau-iter` |
| Implement tableau composition, inversion, gate conjugation, commutation, Pauli products, sign multiplication, random hooks, and text round trips | Satisfied | `crates/stab-core/tests/stabilizers.rs`; `cargo test -p stab-core stabilizers --quiet`; `just oracle::run --milestone M6` |
| Implement single-qubit Clifford gates, two-qubit Clifford gates, swaps, Pauli-product operations, and derived operations used by Stim tests | Satisfied | `crates/stab-core/src/stabilizers/`; `crates/stab-core/src/circuit_tableau.rs`; direct M6 tests and oracle rows |
| Implement conversion helpers needed by later `Circuit::to_tableau`, inverse-circuit, flow, and stabilizer-to-tableau operations | Satisfied | `crates/stab-core/src/circuit_tableau.rs`; `crates/stab-core/src/circuit_inverse.rs`; `crates/stab-core/src/flow.rs`; `crates/stab-core/src/stabilizers/conversions.rs` |
| Add property tests for inverse, identity, associativity, commutation, conjugation, text round trips, and scalar or reference equivalence | Satisfied | `crates/stab-core/tests/stabilizers.rs`; `crates/stab-core/tests/stabilizers_to_tableau.rs`; `just oracle::run --milestone M6`; `cargo test -p stab-core stabilizers --quiet` covers the stabilizer-named property slice |
| `cargo test -p stab-core stabilizers` passes direct and property tests | Satisfied | Command passed for the stabilizer-named property slice; util-top M6 rows are covered by their manifest commands and by `just oracle::run --milestone M6` |
| `cargo test -p stab-core --test stabilizers_vs_amplitudes` passes the M6-owned unitary-to-tableau parity subset | Satisfied | Command passed 5 tests |
| `just oracle::run --milestone M6` passes selected C++ Stim algebra comparisons | Satisfied | Command passed all 17 implemented M6 property and structural rows |
| `just oracle::list --milestone M6` shows implemented M6 rows, and the fixture manifest names owned and deferred util-top subcases | Satisfied | `just oracle::list --milestone M6` lists all M6 rows as implemented with property or structural grouping; `oracle/fixtures/manifest.csv` contains the owned and deferred subcase notes |
| `just bench::compare --milestone M6` reports Pauli, Clifford, tableau, tableau-iterator, and stabilizers-to-tableau workloads with normalized rates and compare notes | Satisfied | `target/benchmarks/m6-completion-compare/compare.json`; strict compare passed all six M6 rows |
| Public algebra APIs avoid Python-hostile lifetime or generic shapes unless documented | Satisfied | M6 public API uses owned `PauliString`, `FlexPauliString`, `CliffordString`, and `Tableau`; public borrowed view parity is explicitly deferred |
| Caller-sized Algebra APIs reject deterministically before unbounded materialization or later algorithmic work | Satisfied by CQ2 hardening | `crates/stab-core/src/stabilizers/limits.rs`; exact resource parents in `oracle/qualification-cases.json`; `cargo test -p stab-core --test cq2_algebra_resources --quiet` |

## Oracle And Benchmark Evidence

- `just oracle::matrix --milestone M6` prints 29 compatibility rows: 17 C++ or util-top implementation rows, six Python semantic-mining rows, and six benchmark rows.
- `just oracle::list --milestone M6` prints seven property rows and ten structural rows, all implemented.
- `just oracle::run --milestone M6` passed every implemented M6 row.
- `just bench::baseline --only M6 --out target/benchmarks/m6-completion-baseline --target-seconds 0.01 --cli-iterations 1` wrote a pinned Stim baseline report.
- `just bench::compare --milestone M6 --baseline target/benchmarks/m6-completion-baseline/baseline.json --strict --report target/benchmarks/m6-completion-compare` wrote a strict compare report with three direct-match rows and three report-only deterministic substitutes, matching the roadmap's M6 benchmark policy.

## Milestone Audit Outcome

- M6 is complete against the current text because direct Rust tests, oracle manifest rows, and benchmark reports cover the owned algebra subset.
- The 2026-06-28 GPT-5.5/xhigh milestone-audit pass initially found that this report omitted GOAL audit/review evidence and overstated the scope of `cargo test -p stab-core stabilizers`.
- The report now clarifies that `cargo test -p stab-core stabilizers` covers the stabilizer-named property slice, while util-top rows are covered through the oracle manifest commands and `just oracle::run --milestone M6`.
- The main residual risk is scope clarity: full Python binding behavior, public borrowed Pauli-string views, exact C++ RNG stream parity, exact uniform tableau sampling, `tableau_to_unitary`, random tableau/unitary round trips, amplitude-simulator cross-checks, full measurement-rich flow checking, and exact random 10k-qubit performance parity are intentionally deferred in `docs/plans/rust-stim-drop-in-rewrite.md`.
- No open M6 under-specification entries remain in `docs/plans/milestone-spec-gaps.md`.

## Full Code Review Outcome

- The 2026-06-28 GPT-5.5/xhigh full-code-review pass found no blocking M6 documentation, workflow, or implementation issues.
- The review found two documentation nits, both fixed here: `just oracle::list --milestone M6` is now cited only for row status and grouping, and manifest notes are cited through `oracle/fixtures/manifest.csv`.
- Large-file review watch list: `crates/stab-core/tests/stabilizers.rs` and `crates/stab-core/src/stabilizers/pauli.rs` are under the 1200-line threshold but remain above the 900-line watch-list threshold for later algebra work.

## Verification Commands

- `cargo test -p stab-core stabilizers --quiet`
- `cargo test -p stab-core --test cq2_algebra_resources --quiet`
- `cargo test -p stab-core --test stabilizers_vs_amplitudes --quiet`
- `just oracle::matrix --milestone M6`
- `just oracle::list --milestone M6`
- `just oracle::run --milestone M6`
- `cargo test -p stab-bench m6_benchmark_rows_have_stab_compare_runners --quiet`
- `just bench::baseline --only M6 --out target/benchmarks/m6-completion-baseline --target-seconds 0.01 --cli-iterations 1`
- `just bench::compare --milestone M6 --baseline target/benchmarks/m6-completion-baseline/baseline.json --strict --report target/benchmarks/m6-completion-compare`
