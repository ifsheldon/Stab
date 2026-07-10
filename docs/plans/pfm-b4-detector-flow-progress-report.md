# PFM-B4 Detector Utility And Flow Solver Progress Report

## Status

PFM-B4 is complete as of 2026-07-11 at implementation commit `0f47eee04eacec96ed4e03dd36a18f58b76a0afc`.
The initial milestone audit and GPT-5.6/max review found correctness, selector, source-attribution, repeat-test, benchmark-density, and documentation issues. The first focused re-review then found `MPAD` simulation-width drift, unrelated-instruction engine selection, quadratic idle-width unitary-repeat setup, sweep-only feedback-transform rejection, and a benchmark contract that did not exercise measurement-signature elimination. A second core review found ignored-only repeat and row-cap drift, non-`CZ` sweep-only feedback acceptance, asymmetric `MPAD` record ordering, valid duplicate reset and measure-reset rejection, and an idle-qubit work-count error. The compatibility review then exposed a vacuous duplicate-observable solver test and stale generator-versus-transform scope wording. All core, benchmark, and compatibility finding-closure reviews are clean, and both required allocation reports record the implementation commit with `local_modifications=false`.

PFM-B4 owns 49 cases in `docs/plans/blocker-closure-ledger.json`: two detecting-region evidence-close cases, fourteen missing-detector evidence-close cases, and thirty-three flow-engine cases.
All forty-nine cases now have behavior or evidence-close status in `stab-core`, exact one-test Cargo selectors, and existing focused oracle evidence.
The previous eleven missing-detector and twenty-eight flow shared-selector debts are zero.

## Frozen Scope

Detecting regions close from the pinned C++ `circuit_to_detecting_regions.simple` case and Python `test_detecting_regions_fails_on_anticommutations_at_start_of_circuit` case plus the already promoted filters, generated-code, gauge, Clifford, feedback, sweep, product-measurement, repeat, and resource evidence.
Missing detectors close from independently selectable subcases inside C++ `missing_detectors.circuit`, plus `big_case_honeycomb_code` and `toric_code_global_stabilizer_product`, while retaining the already promoted row-reduction, observable, MPAD, Clifford, SPP, repeat, and resource evidence.
Behavior outside those exact evidence-close sets is not implementation scope unless a failing pinned case or separately approved requirement selects it.

Flow closure owns the twenty-four stable examples extracted from C++ `circuit_flow_generators.various`, the generated `all_operations` invariant, the C++ empty, simple, and repetition-code solver examples, the four selected Python generator or solver tests, four retained checker oracle rows, and one new supported over-sixteen-measurement solver case.
Python binding shape, signed Python APIs, reverse-flow transforms owned by PFM-B1, and unselected feedback remain outside this milestone.

## Solver Decision

The previous solver reduced query Pauli terms against `circuit_flow_generators`, which was already polynomial, but silently switched to exhaustive measurement-subset enumeration if generator construction returned an error.
PFM-B4 removes that fallback entirely.
Supported circuits use the generator-row GF(2) basis for every measurement count, while unsupported circuits preserve the generator's typed error instead of hiding it behind a sixteen-measurement cliff or an exponential search.

The retained eliminator uses deterministic input-X, input-Z, output-X, and output-Z pivot order, canonical generator order, symmetric-difference measurement rows, and a deterministic greedy measurement-count reduction heuristic.
Selected pinned examples retain exact measurement-index results, while underdetermined cases require a checker-valid semantic solution because Stim documents that non-unique solutions may differ and its internal solver reduces a pre-finalized reverse table instead of Stab's public canonical generator list.
The flow generator and solver continue to share `MeasurementFeedbackFlowSolver` transitions.
Generator construction and the sparse reverse tracker now also share the typed `ReverseFlowTransition` classifier for measurement, reset, measure-reset, pair-measurement, MPP, MPAD, heralded-record, SPP, controlled-Pauli, sweep-no-op, detector, observable, tableau, ignored, and unsupported families.
Unsigned flow checking retains sparse reverse-tracker state because it must validate caller-supplied measurement and observable terms and folded repeats without materializing a generator table; fixed-seed cross-engine tests prove agreement instead of forcing both algorithms through an unsuitable common storage type.
Single-instruction reset and measure-reset fast paths now transform a full identity basis so untouched lower-index qubits retain valid identity flows.
Queries extending beyond the circuit use an implicit identity suffix instead of materializing two full-width generator rows per missing qubit, preventing sparse high-index queries from amplifying into quadratic storage.
Controlled-Pauli reverse traversal now processes each target pair independently: record-to-qubit feedback toggles measurement parity, plain-qubit pairs apply their tableau, and accepted classical-only pairs are semantic no-ops, matching pinned Stim mixed-group behavior.
Reverse-solver selection now follows the instruction transition itself. Pure ignored annotation or ordinary-noise circuits use a folded identity path that does not expand repeats or inherit measurement-row caps, mixed unitary-plus-noise circuits use tableau propagation with noise ignored, and measurement-rich circuits still traverse ignored operations as no-ops.
Flow generation and solving use an internal simulated-qubit count that excludes `MPAD` pad values while preserving Stim's public `Circuit.num_qubits` behavior, which still counts numeric pad targets.
Single and composed `MPAD` generation reverse pad values against forward record indices like pinned Stim, including asymmetric `MPAD 1 0` cases.
Reset generators deduplicate repeated reset targets, while measure-reset generators preserve the final reset and input-measurement row plus deterministic parity rows from each earlier duplicate record to the final record, including inversion signs.
Supported unitary-repeat folding builds a dense transform over only body-touched qubits and leaves all other tracker slots as implicit identity, including sparse high-index active qubits and wide idle suffixes.
Feedback inlining continues to reject record-only classical groups in its narrower transform contract, preserves sweep-only `CZ` groups unchanged like pinned Stim, and rejects sweep-only `CX`, `CY`, `XCZ`, and `YCZ` groups.

## Tests

- Give all 49 ledger cases exact one-test Cargo selectors enforced by the oracle harness.
- Split the twenty-four `circuit_flow_generators.various` examples into exact per-example tests without changing their pinned outputs.
- Split C++ and Python solver groups into stable empty, simple, repetition-code, measured-idle, multi-target, and fewer-measurements selectors.
- Split all selected `missing_detectors.circuit` outputs into exact per-subcase tests.
- Add a supported circuit with at least 33 measurements that has both a many-single-measurement solution and a one-product-measurement solution; require the pinned greedy one-measurement result and verify the reconstructed flow with the unsigned checker.
- Add rank-deficient, inconsistent, underdetermined, sparse-high-qubit, nonempty ignored query-term, and unsupported-circuit solver tests. Retain duplicate measurement and observable parity in the `Flow` value-object and checker suites that consume those terms.
- Add a fixed-seed generated differential corpus whose supported circuits produce generators accepted by the unsigned checker and whose non-empty Pauli projections can be solved back into checker-accepted flows.
- Add all 255 non-empty unsigned two-qubit Pauli endpoint queries over a bounded circuit matrix and compare both solvable and unsolvable outcomes with exhaustive checker-backed measurement subsets.
- Add direct reset and measure-reset idle-qubit regressions, a 65,536-qubit sparse query suffix regression, repeat-versus-expanded solving, and direct repeat-cap rejection.
- Lock the generated all-operations fixture to the pinned signed 40-flow set, independent of the unsigned checker.
- Add pinned regressions for high-index and million-repeat ignored-only identity generators, mixed unitary-plus-noise generators, measurement-free mixed sweep-plus-quantum groups, `MPAD` simulation width, asymmetric simple and composed `MPAD` record ordering, duplicate reset and measure-reset targets with inversion parity, solving through `MPAD`, gate-specific sweep-only feedback preservation or rejection, Stim-compatible flow ordering, and touched-qubit unitary-repeat transforms with 65,536-wide idle and active suffix cases.
- Retain detecting-region and missing-detector negative and resource suites.

## Oracle Evidence

The focused Rust-test proxy rows are `pfm-b4-detecting-regions-simple-rust`, `pfm-b4-detecting-regions-start-anticommutation-rust`, `pfm-b4-missing-detectors-circuit-rust`, `pfm-b4-missing-detectors-honeycomb-rust`, `pfm-b4-missing-detectors-toric-rust`, `pfm-b4-flow-generators-various-rust`, `pfm-b4-flow-solver-cpp-rust`, `pfm-b4-flow-solver-python-rust`, and `pfm-b4-flow-solver-matrix-rust`.
The proxies use structural execution comparators because the oracle runner observes Cargo pass or fail, while the selected Rust tests contain literal exact output assertions where order and text are contractual.
Every PFM-B4 ledger selector now carries an explicit `--exact` contract, the oracle harness requires exactly one listed test for such selectors, and prefix-colliding test names no longer count as independent evidence.
The flow blocker also freezes four retained checker oracle signatures and the report-only `pf5-has-all-flows-batch` benchmark instead of relying on prose to preserve those dependencies.

## Benchmark Evidence

Add contract-only row `pfm-b4-flow-solve-matrix-sizes` with deterministic measurement-rich scrambled dense `32x64` and `128x256` Pauli bases carrying exact 7- and 24-singleton measurement-signature sets plus a `512x1024` high-qubit Pauli basis carrying an exact 12-singleton measurement-signature set and scrambled within exactly 32 sparse active qubits.
Every workload circuit contains one controlled-Pauli instruction that mixes a classical-feedback target group with a plain two-qubit target group. The pre-PFM-B4 generator rejected that shape and entered the removed exhaustive fallback, so the 24-measurement medium case also proves operation beyond the former sixteen-measurement fallback cap.
The row uses 17, 65, and 33 queries composed from three generator rows with distinct singleton measurement signatures and requires nonempty solved parity.
The runner enforces the exact singleton-signature set, at least 15% overall density for dense bases, at most 8% overall density for the sparse basis, a 15% through 85% active-submatrix density band, exact active measurement-bearing Pauli support, and the mixed controlled-Pauli shape. It pins literal work values, executes production case construction in tests, validates all expected solutions before sampling, times only end-to-end public solver work plus black-box result consumption, and reports query-inclusive Pauli input bits per second, solved queries per second, peak live allocation, and sampled resident delta.
The row remains non-primary and report-only because pinned Stim exposes no faithful in-process baseline without Python binding overhead.
Existing detecting-region and missing-detector rows remain report-only and require no new workload because this milestone changes their evidence granularity, not their production paths.
The existing report-only `pf6-sparse-rev-frame-loop` row gains a unitary-repeat submeasurement with one active qubit and 65,535 idle qubits in a 65,536-wide flow so the touched-qubit transform fix has allocation and timing evidence.

The superseded Z-only probe under `target/benchmarks/pfm-b4-flow-solver-probe` and unitary direct-generator probe under `target/benchmarks/pfm-b4-flow-solver-redesign-probe` do not exercise the final benchmark contract and are not milestone evidence.
The corrected one-run exploratory allocation probes under `target/benchmarks/pfm-b4-flow-solver-measurement-rich-probe` and `target/benchmarks/pfm-b4-sparse-repeat-high-idle-probe` record `local_modifications=true`, so they validate runner behavior but are not completion evidence:

| Workload | Median | Normalized rate | Peak live allocated bytes | Resident delta |
| --- | ---: | ---: | ---: | ---: |
| Measurement-rich dense `32x64`, 7 signatures, 17 queries | 4.480532 milliseconds | 699,900 input bits/s; 3,794 queries/s | 80,512 | 0 |
| Measurement-rich dense `128x256`, 24 signatures, 65 queries | 79.781380 milliseconds | 619,300 input bits/s; 814.7 queries/s | 321,088 | 0 |
| Measurement-rich sparse `512x1024`, 12 signatures, 33 queries | 211.402115 milliseconds | 2,640,000 input bits/s; 156.1 queries/s | 673,208 | 0 |
| Folded unitary repeat with 65,535-qubit idle suffix in a 65,536-wide flow | 589.462743 milliseconds | 111,200 idle qubits/s | 3,293,377 | 0 |

Clean committed-HEAD allocation reports for both changed report-only rows were recorded from `0f47eee04eacec96ed4e03dd36a18f58b76a0afc` with `local_modifications=false`, release profile, warmup enabled, three measurement runs, and zero sampled resident delta. Paired matrix-bit and query measurements reuse the same timing sample, so their duration and memory evidence intentionally match.

| Clean committed workload | Median | Normalized rate | Peak live allocated bytes | Resident delta |
| --- | ---: | ---: | ---: | ---: |
| Measurement-rich dense `32x64`, 7 signatures, 17 queries | 4.539794 milliseconds | 690,800 input bits/s; 3,745 queries/s | 80,512 | 0 |
| Measurement-rich dense `128x256`, 24 signatures, 65 queries | 80.954289 milliseconds | 610,300 input bits/s; 802.9 queries/s | 321,088 | 0 |
| Measurement-rich sparse `512x1024`, 12 signatures, 33 queries | 213.161648 milliseconds | 2,618,000 input bits/s; 154.8 queries/s | 673,208 | 0 |
| Folded unitary repeat with 65,535-qubit idle suffix in a 65,536-wide flow | 578.529221 milliseconds | 113,300 idle qubits/s | 3,293,377 | 0 |

The authoritative reports are `target/benchmarks/pfm-b4-flow-solver-clean/report.md` and `target/benchmarks/pfm-b4-sparse-repeat-clean/report.md`.

## Verification To Date

```text
cargo test -p stab-core --test pfm_b4_flow_evidence --test pfm_b4_flow_solver --test pfm_b4_missing_detector_evidence --quiet
cargo test -p stab-core --test circuit_flow_generators --test circuit_flows --quiet
cargo test -p stab-core detecting_regions --quiet
cargo test -p stab-core missing_detectors --quiet
cargo test -p stab-core sparse_rev_frame_tracker --quiet
cargo test -p stab-bench detector_utility_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-oracle fixtures --quiet
cargo clippy -p stab-core -p stab-bench -p stab-oracle --all-targets -- -D warnings
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
just oracle::blockers --check-selectors
just oracle::run --milestone PF5
just oracle::run --implemented-only
just bench::smoke
just bench::compare-allocations --only pfm-b4-flow-solve-matrix-sizes --measurement-runs 1 --report target/benchmarks/pfm-b4-flow-solver-measurement-rich-probe
just bench::compare-allocations --only pf6-sparse-rev-frame-loop --measurement-runs 1 --report target/benchmarks/pfm-b4-sparse-repeat-high-idle-probe
```

Completion evidence was regenerated from clean committed `HEAD` with:

```text
just bench::compare-allocations --only pfm-b4-flow-solve-matrix-sizes --warmup --measurement-runs 3 --report target/benchmarks/pfm-b4-flow-solver-clean
just bench::compare-allocations --only pf6-sparse-rev-frame-loop --warmup --measurement-runs 3 --report target/benchmarks/pfm-b4-sparse-repeat-clean
```

## Milestone Audit

The post-correction milestone audit finds no remaining implementation, test, oracle, benchmark-contract, review, documentation, or clean-evidence defect in the current PFM-B4 scope. PFM-B4 is complete.

| Requirement | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Detecting-region evidence closure | Satisfied | Two exact ledger selectors; `pfm-b4-detecting-regions-simple-rust`; `pfm-b4-detecting-regions-start-anticommutation-rust` | Existing negative, generated, filter, and resource suites remain green. |
| Missing-detector evidence closure | Satisfied | Fourteen exact ledger selectors; three focused PFM-B4 oracle rows | Named C++, honeycomb, and toric cases are independently executable without speculative generated-code expansion. |
| Shared flow foundation and fallback removal | Satisfied | `circuit_flow/transitions.rs`; `circuit_flow/generators.rs`; `circuit_flow/solver.rs`; `pfm_b4_flow_solver_rejects_invalid_circuits_without_exhaustive_fallback` | Supported circuits use generator-table GF(2) elimination; no production exhaustive subset path remains. |
| Deterministic Stim-compatible flow semantics | Satisfied | `pfm_b4_flow_evidence`; `circuit_flow_generators`; `stabilizer_flows` | Exact pinned rows, mixed controlled-Pauli groups, ignored-only folding, asymmetric MPAD, duplicate reset semantics, and flow ordering are covered. |
| Property, differential, and resource behavior | Satisfied | `pfm_b4_flow_solver`; `circuit_flows`; `stabilizer_flows`; `sparse_rev_frame_tracker::unitary_repeat` | Includes fixed-seed generation, all 255 bounded queries, Pauli-projection semantics for nonempty ignored query terms, duplicate value-object parity, over-sixteen solving, repeat caps, sparse suffixes, and 65,536-wide tracker regressions. |
| Oracle attribution and selector independence | Satisfied | `just oracle::blockers --check-selectors`; `just oracle::run --milestone PF5`; `just oracle::run --implemented-only` | All 49 PFM-B4 cases resolve independently; retained checker evidence is named separately. |
| Matrix and sparse-repeat benchmark contracts | Satisfied | `pfm-b4-flow-solve-matrix-sizes`; `pf6-sparse-rev-frame-loop`; 77 `stab-bench` tests; `just bench::smoke` | Workload construction, semantic guards, timing boundary, work units, and dirty allocation probes pass. |
| Module-size and ownership guardrails | Satisfied | `circuit/counts.rs`; `circuit_flow/generators/canonicalize.rs`; large-file scan | Changed production Rust files remain below 1,200 lines after extracting qubit counting and canonical row reduction. |
| GPT-5.6/max review closure | Satisfied | Core, benchmark, and compatibility finding-closure reviews report no remaining findings | Compatibility probes confirmed the signed 40-flow set, Pauli-projection solver boundary, MPAD ordering, controlled-Pauli handling, feedback gating, and duplicate generator-versus-transform scope. |
| Clean committed-HEAD allocation evidence | Satisfied | `target/benchmarks/pfm-b4-flow-solver-clean/report.md`; `target/benchmarks/pfm-b4-sparse-repeat-clean/report.md` | Both reports identify `0f47eee04eacec96ed4e03dd36a18f58b76a0afc`, `local_modifications=false`, warmup, three measurement runs, allocation maxima, and zero resident delta. |

The matrix-workload loophole revealed during review is resolved in `docs/plans/milestone-spec-gaps.md` with an exact mixed controlled-Pauli shape, signature sets, timing boundary, density guards, support guard, and production-construction test. The later solver-query loophole is also resolved there by defining Stim-compatible Pauli-projection semantics and moving duplicate measurement or observable parity evidence to value-object and checker tests that consume those terms. No additional PFM-B4 specification loophole remains open from this audit.

## Completion Gate

PFM-B4 satisfies its completion gate: all 49 selectors resolve independently, the ledger and focused oracle rows validate, no production exhaustive measurement-subset path remains, every generated solution passes the unsigned checker, both clean allocation reports identify committed `HEAD`, milestone-audit and GPT-5.6/max full-code-review findings are fixed, and the PFM5 detecting-region, missing-detector, and flow-engine specification entries are resolved.
