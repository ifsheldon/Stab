# PFM-B1 Reverse-Flow Progress Report

## Status

The PFM-B1 implementation and source-owned evidence are in place as of 2026-07-11.
All nineteen cases in the `pfm2-qec-transforms` blocker ledger are marked implemented, every case has a distinct exact Cargo selector, and the ledger reports zero planned cases and zero shared selectors.
All findings from the first GPT-5.6/max review, its focused re-review lanes, and the adversarial confirmation passes are fixed, and the final focused recheck reports no remaining P0 through P2 blocker. Four clean allocation reports from committed `HEAD=4f193f19cebf132f7baf0a3aa1cc799a153a71ed` complete the remaining evidence gate.

## Implemented Architecture

`crates/stab-core/src/circuit_inverse/reverse_flow.rs` replaces the former measurement-rich packet recognizers with one tracker-driven reverse-flow engine.
The engine assigns each input `Flow` a typed reverse state containing Pauli input, Pauli output, measurement parity, observable parity, and a synthetic tracker target.
It dispatches through `ReverseFlowTransition`, reuses `SparseReverseFrameTracker` for Clifford and collapse propagation, and owns detector and observable measurement remapping, coordinate shifts, tags, output-flow construction, and batched unsigned output-flow validation.

The selected implementation boundary is:

- Supported through gate-family handlers: Clifford tableau gates, controlled-Pauli gates without measurement-record feedback, `SPP`, `SPP_DAG`, `M`, `MX`, `MY`, `R`, `RX`, `RY`, `MR`, `MRX`, `MRY`, `MXX`, `MYY`, `MZZ`, `MPP`, `MPAD`, `DETECTOR`, `OBSERVABLE_INCLUDE`, `TICK`, `QUBIT_COORDS`, `SHIFT_COORDS`, and ordinary noise handled by the shared transition classifier.
- Pure unitary repeats remain folded through the existing sparse cycle validation path.
- Measurement-rich repeats use generic flattened reverse traversal after a checked one-million-instruction work calculation; larger expansions fail before traversal.
- Recursively instruction-empty repeat bodies are skipped without iterating their repeat count.
- Pure-unitary tableau validation is limited to 8,192 qubits and falls back to sparse folded validation above that memory budget; empty-flow unitary reversal bypasses validation entirely.
- Returned flows share one sparse reverse traversal with a distinct synthetic target per flow instead of multiplying circuit work by flow count.
- Distinct absolute and relative measurement terms that resolve to one record reject before tracker XOR cancellation, matching pinned Stim v1.16.0.
- Measurement-record feedback and heralded record reversal remain fail-closed.
- Duplicate measurement, reset, measure-reset, and pair-measurement qubit targets remain fail-closed under `docs/plans/pfm2-time-reverse-duplicate-target-boundary-scope.md`.
- Non-finite probability or coordinate arguments remain impossible after the validated circuit-construction boundary and are also rejected by reverse-flow preflight if an internal caller violates that invariant.

The packet-specific `flow_flip`, single-instruction measurement, MZZ-suffix, and MPAD record-tail time-reversal dispatch was removed from `crates/stab-core/src/circuit_inverse.rs`.
The separately scoped `Circuit::inverse_qec` packet implementation remains unchanged because PFM-B1 owns general `time_reversed_for_flows` semantics, not an unbounded expansion of the no-flow inverse API.

## Case Evidence

The nineteen ledger cases resolve as follows:

| Source family | Cases | Evidence |
| --- | ---: | --- |
| Pinned C++ inverse-QEC cases | 4 | Anticommutation, single-measurement flow reversal, four MZZ flow relationships, and flow support beyond circuit qubits have distinct selectors in `crates/stab-core/tests/circuit_inverse_qec.rs`. |
| Pinned Python `test_inv_circuit` cases | 8 | Empty identity, reset-H-MX-detector, measurement-to-reset, reset-to-measurement, kept measurement, option handling, noisy measure-reset, and feedback rejection have distinct selectors across `circuit_inverse_qec.rs` and `circuit_inverse_qec_pfm_b1.rs`. |
| Pinned Python generated and ordering cases | 6 | Rotated surface-code reversal, extra flow qubits, M ordering, MZZ ordering, MR ordering, and tagged Pauli observables have distinct selectors. |
| Source-owned MPAD matrix | 1 | `pfm_b1_mpad_flow_matrix` covers constants `0` and `1`, absolute and relative records, observable-only and mixed terms, duplicate observable-id parity, and Clifford gates on both sides. |

Additional source-owned evidence includes:

- `pfm_b1_measurement_rich_repeat_uses_bounded_expansion` for successful bounded expansion and pre-traversal rejection above one million instructions.
- `pfm_b1_instruction_empty_nested_repeat_skips_repeat_count_work` for a billion-count recursively empty repeat followed by real work.
- `pfm_b1_high_qubit_unitary_validation_uses_sparse_memory` for empty and nonempty-flow reversal at qubit 1,000,000 without dense tableau allocation.
- `pfm_b1_output_flow_validation_batches_many_flows` for 1,024 returned flows through one batched checker traversal.
- `pfm_b1_absolute_relative_record_aliases_match_stim_rejection` for the pinned alias-collision boundary.
- `pfm_b1_sweep_control_target_order_matches_stim` for the pinned Stim v1.16.0 target-order matrix across `CX`, `CY`, `CZ`, `XCZ`, and `YCZ`, including the accepted `CZ sweep[0] sweep[1]` case and rejection when a sweep target occupies a gate's qubit-only side.
- `pfm_b1_supported_flow_reversal_is_semantically_involutive`, a 48-case property run over small generated Clifford, measurement, pair-measurement, MPAD, SPP, and tick circuits using generated satisfiable flows and unsigned checker validation after both reversals.
- Existing negative tests for anticommutation, unsatisfied flows, out-of-range records, feedback, duplicate targets, and unsupported heralded records.
- Existing exact tests for noisy measurements, noisy measure-resets, multi-instruction measurement flow, MZZ suffixes, MPAD observables, flow ordering, tags, and coordinates.

## Oracle Evidence

The PF2 oracle manifest includes thirteen independently runnable exact-output rows for every ledger case whose comparator is `exact`, plus one exact empty-stdout/error-class row for absolute-relative record alias rejection.
`oracle::record --check-clean` now builds a source-owned C++ helper against the pinned `vendor/stim` static library and regenerates every `core-time-reverse-flows` row from Stim v1.16.0 instead of trusting Stab to define its own goldens.
Each ledger pinned-golden signature binds the case id, fixture-relative corpus path, expected-output path, and SHA-256 digests of both files; comparator compatibility is enforced for pinned-golden rows.
Direct Cargo-test fixtures require at least one passed test and exactly one passed test for `--exact`, so an ignored or over-broad test cannot satisfy evidence.
The three Python measurement-ordering invariants have separate exact Cargo-test proxy rows with Python source provenance, while generated-surface reversal and feedback rejection retain structural/error-class evidence.
The aggregate implementation rows remain useful for broader regression coverage:

- `pfm-b1-time-reverse-general-rust`
- `pfm-b1-surface-code-reversal-rust`
- `pfm-b1-mpad-flow-matrix-rust`

The existing `pf2-time-reverse-flow-unitary-rust`, `pf2-time-reverse-flow-measurement-rust`, and `pf2-time-reverse-flow-mzz-unitary-suffix-rust` rows are synchronized with the general engine.
`just oracle::run --milestone PF2 --exact` passes all fourteen exact rows.
`just oracle::blockers --check-selectors` reports `19 implemented`, `0 planned`, `0 shared-selectors`, nineteen exact selectors, and four source-owned supporting benchmarks for PFM-B1.

## Benchmark Evidence

The report-only benchmark family is split by semantic workload:

| Row | Timed work | Normalized unit | Resource purpose |
| --- | --- | --- | --- |
| `pf2-time-reverse-flow-measurement` | Historical 24-case pinned measurement-rich corpus through the general engine | flows/s | Preserves the established measurement key and report identity. |
| `pfm-b1-time-reverse-generated-surface` | Repeat-free rotated-memory-X matrix at distance 3/rounds 2, distance 5/rounds 2, and distance 7/rounds 2; generation, repeat absence, and literal 66, 130, and 226 compact source-instruction checks occur outside timing | source-instructions/s | Measures allocation growth against circuit state and compact source size without hiding repeat expansion. |
| `pfm-b1-time-reverse-mpad-matrix` | Seven-flow semantic matrix plus 1, 8, and 64 independent MPAD record-flow points | flows/s | Measures sparse record and observable parity while exposing linear instruction-and-flow scaling. |
| `pfm-b1-time-reverse-large-unitary-repeat` | Identity unitary repeats at counts 1, 1,024, and 1,000,000,000 plus a billion-repeat eight-qubit wider body; repeat count and parsed body work are checked outside timing | transforms/s | Separates logarithmic count work from body/state-size allocation without claiming expanded-operation throughput. |
| `pfm-b1-time-reverse-sparse-high-qubit` | Otherwise identical unitary operations at qubit 0 and qubit 1,000,000 with the same nonempty low-width validation flow | transforms/s | Detects maximum-index allocation amplification in both reversal and validation. |

The pre-review dirty-worktree probes used obsolete one-point measurement contracts and are superseded by the matrices above.
Feature-gated allocation tests enforce low-versus-million index deltas, logarithmic repeat-count work with bounded peak live bytes, three-point compact-body/state-size slopes at widths 1, 4, and 16, and three-point MPAD slopes at 8, 64, and 1,024 flows. A synthetic profile proves that the shared acceptance function rejects a retained dense quadratic matrix.

## Clean Committed Evidence

All four required reports were recorded from committed `HEAD=4f193f19cebf132f7baf0a3aa1cc799a153a71ed` with `local_modifications=false`, warmup enabled, and three measurement runs. The JSON reports retain per-measurement variance, allocation counts and bytes, sampled resident bytes, and resident deltas; the table below summarizes the medians and maximum resource observations.

| Report | Median measurements | Maximum peak live allocation | Maximum sampled resident delta |
| --- | --- | ---: | ---: |
| `target/benchmarks/pfm-b1-generated-surface-clean/compare.json` | d3/r2 30.750 us; d5/r2 84.638 us; d7/r2 171.600 us | 84,280 bytes | 0 bytes |
| `target/benchmarks/pfm-b1-mpad-matrix-clean/compare.json` | matrix 4.874 us; scale 1 0.750 us; scale 8 3.532 us; scale 64 29.238 us | 44,672 bytes | 8,192 bytes |
| `target/benchmarks/pfm-b1-large-repeat-clean/compare.json` | count 1 5.716 us; count 1,024 7.586 us; count 1 billion 12.870 us; wide body 215.680 us | 19,496 bytes | 0 bytes |
| `target/benchmarks/pfm-b1-sparse-high-qubit-clean/compare.json` | qubit 0 1.414 us; qubit 1,000,000 0.972 us | 2,120 bytes | 0 bytes |

These rows remain `contract-only`, `non-primary-report-only` evidence because the harness has no faithful in-process pinned-Stim comparator for these Rust transform workloads. No timing ratio or primary threshold is claimed.

## Verification To Date

The following commands pass on the implementation worktree:

```text
cargo clippy -p stab-core --all-targets -- -D warnings
cargo test -p stab-core --quiet
cargo test -p stab-core --test circuit_inverse_qec_pfm_b1 --quiet
cargo test -p stab-bench pf2_transform_benchmark_rows_have_stab_compare_runners --quiet
cargo test -p stab-bench --features count-allocations pfm_b1::allocations --quiet
just oracle::blockers --check-selectors
just oracle::record --check-clean
just oracle::run --milestone PF2
just oracle::run --milestone PF2 --exact
just bench::smoke
```

The clean reports were recorded with:

```text
just bench::compare-allocations --only pfm-b1-time-reverse-generated-surface --warmup --measurement-runs 3 --report target/benchmarks/pfm-b1-generated-surface-clean
just bench::compare-allocations --only pfm-b1-time-reverse-mpad-matrix --warmup --measurement-runs 3 --report target/benchmarks/pfm-b1-mpad-matrix-clean
just bench::compare-allocations --only pfm-b1-time-reverse-large-unitary-repeat --warmup --measurement-runs 3 --report target/benchmarks/pfm-b1-large-repeat-clean
just bench::compare-allocations --only pfm-b1-time-reverse-sparse-high-qubit --warmup --measurement-runs 3 --report target/benchmarks/pfm-b1-sparse-high-qubit-clean
```

## Milestone Audit

The milestone audit completed its final pass on 2026-07-11.

Finding closure:

- The audit found one evidence-maintenance defect: `blocker_ledger_requires_planned_selectors_for_planned_cases` named a PFM-B1 case whose lifecycle changed from planned to implemented. The test now selects an arbitrary currently planned case, so it verifies the schema invariant without coupling to a milestone's progress.
- The compatibility audit found that sweep-controlled Clifford instructions were incorrectly kept on the pure-unitary path and that target-side validity was not enforced for asymmetric controlled gates. Any instruction containing a classical target now uses the general reverse-flow engine, and an exact pinned-Stim matrix verifies the accepted and rejected sweep-target orders for `CX`, `CY`, `CZ`, `XCZ`, and `YCZ`.
- GPT-5.6/max review found maximum-qubit-index allocation amplification in `SparseReverseFrameTracker`. Qubit sensitivities now use sparse maps, compact flow validation avoids circuit-width identity allocation, and direct storage plus end-to-end million-index regressions protect the resource contract.
- GPT-5.6/max review found that non-pair MPP, SPP, and correlated-error targets used group reversal instead of Stim's raw-target reversal. The shared inverse helper now reverses raw targets for non-pair gates and two-target groups for pair gates, with exact regressions for both classes.
- GPT-5.6/max review found that inverted `M`/`MX`/`MY` and noisy inverted `MR`/`MRX`/`MRY` paths succeeded where Stim rejects synthesized invalid targets. Modifiers now survive synthesis until normal instruction validation produces the pinned error class.
- GPT-5.6/max review found that exact ledger cases used aggregate structural proxies and unique filters. Thirteen pinned-golden rows, three focused Python ordering rows, nineteen `--exact` selectors, and four frozen supporting-benchmark signatures now make those claims executable.
- GPT-5.6/max review found that one-point allocation probes and a renamed historical measurement overstated evidence. The historical key is restored, scaling matrices and exact work-unit tests are source-owned, and the report no longer claims count-independent timing.
- Focused GPT-5.6/max core re-review found dense tableau allocation at high qubit ids, count-proportional traversal of recursively empty repeats, one full output-circuit traversal per returned flow, and absolute-relative record alias cancellation that diverged from Stim. Empty flows now skip validation, tableau construction has an explicit qubit budget, sparse checking is batched, empty nested repeats are skipped recursively, aliases reject before seeding, and direct regressions cover every boundary.
- Focused GPT-5.6/max evidence re-review found that reverse-flow goldens could drift with Stab, ignored tests counted as execution, pinned-golden comparator compatibility was not enforced, and three Python ordering rows claimed exact ordering beyond upstream. A pinned C++ recorder, path and digest bindings, passed-test accounting, comparator checks, and semantic-only ordering selectors close those defects.
- Focused GPT-5.6/max benchmark re-review found an expanded generated repeat mislabeled as compact source work, report-only matrices without executable acceptance bounds, an empty-flow sparse row, a fixed-size MPAD point, and stale rollup prose. The generated matrix is now repeat-free, the sparse row has a nonempty flow, MPAD has 1/8/64 scale points, allocation-delta tests enforce the contracts, and the inventory, roadmap, and rollup are synchronized.
- The adversarial GPT-5.6/max confirmation pass found that observable terms were checked for anticommutation before GF(2) combination, strict reversal-only record-alias rejection leaked into the ordinary unsigned checker, and empty checker batches could still select a dense tableau. Observable effects now map directly onto each flow marker before reverse traversal, ordinary checking XOR-cancels aliases while reversal rejects them at its own boundary, empty batches return immediately, and mixed-batch regressions lock the distinctions.
- The confirmation pass found that exact process evidence could accept a matching one-mebibyte prefix after capture truncation, a symlinked fixture-root parent could be accepted, and reverse-flow corpus input had no aggregate bound. Process truncation now fails before comparison, source-owned files are opened through descriptor-relative no-follow component walks, fixture reads and helper protocols have one-mebibyte limits, and focused mutation-style tests cover truncation, a symlinked root, and oversized input.
- The confirmation pass found that endpoint allocation multipliers could admit quadratic growth. Repeat-body and MPAD gates now use three-point incremental slopes, MPAD allocation-only coverage extends to 1,024 flows, and a negative synthetic profile proves that a retained dense `u64` matrix fails the same acceptance function.
- Post-fix core confirmation found that the tableau fast path rejected valid flows whose Pauli width differed from the circuit width, even though Stim treats qubits outside the circuit as idle. Width-mismatched batches now use the sparse checker, with boolean and diagnostic regressions for both shorter and million-index longer flows.
- Post-fix evidence confirmation found that recording could truncate an outside inode through a hard-linked golden, child side-output reads retained a check/use race and unbounded reopen, and compatibility-matrix reads remained unbounded. Golden writes now use an exclusive same-directory temporary file and descriptor-relative atomic rename, side outputs are opened once without following links and read through a one-mebibyte limit, compatibility-matrix readers share the same bound, and focused hard-link, symlink, and oversized-file tests lock the contracts.
- Post-fix benchmark confirmation found that saturating slope arithmetic could fail open on synthetic overflow and that one planning summary mislabeled the sparse-index additive-delta check. Acceptance arithmetic now uses checked products and rejects overflow, while the planning text distinguishes MPAD and repeat incremental slopes from the sparse-index delta gate.
- Final resource confirmation found that auxiliary-output limits were enforced only after child exit and that per-run scratch directories accumulated. Fixture processes now monitor side-output descriptors every process poll, terminate the full process group as soon as a file exceeds one mebibyte or becomes unsafe, and own each run directory through an RAII guard that removes it on success and every error path.
- No implementation, evidence, benchmark, documentation, or operational defect remains from the final audit.
- No PFM-B1 specification loophole was revealed. The one-million-instruction measurement-rich repeat cap, heralded-record rejection, duplicate-target hardening, observable-parity behavior, benchmark timing boundaries, normalized work, live side-output limit, scratch cleanup, and clean-evidence commands are explicit in this report and the public API documentation.

Completion matrix:

| Requirement | Status | Evidence |
| --- | --- | --- |
| Nineteen owned pinned subcases | Satisfied | `just oracle::blockers --check-selectors` reports `19 implemented`, `0 planned`, and `0 shared-selectors`. |
| General gate-family reverse engine | Satisfied | `crates/stab-core/src/circuit_inverse/reverse_flow.rs`; packet recognizers were removed from `circuit_inverse.rs`. |
| Shared Clifford and sparse tracking | Satisfied | `ReverseFlowTransition` dispatch plus `SparseReverseFrameTracker` propagation. |
| Measurement, reset, pair, MPP, MPAD, detector, and observable semantics | Satisfied | Exact tests in `circuit_inverse_qec.rs`, `circuit_inverse_qec_mpad.rs`, `circuit_time_reverse_flow_mzz_suffix.rs`, and `circuit_inverse_qec_pfm_b1.rs`. |
| Feedback and duplicate hardening | Satisfied | `pfm_b1_feedback_rejection` and the existing duplicate measurement, reset, measure-reset, and pair-measurement tests. |
| Repeat and hostile-input boundaries | Satisfied | Billion-iteration folded-unitary tests and benchmark, checked one-million-instruction measurement-rich cap, heralded-record rejection, and non-finite argument boundary tests. |
| Returned-flow validation | Satisfied | Both general and pure-unitary paths validate all returned flows in one sparse traversal; the 1,024-flow regression and property test cover the batch path. |
| Generated-surface structural parity | Satisfied | `pfm_b1_surface_code_reversal` compares all detector and observable detecting-region signatures under reversed tick indexing. |
| Oracle evidence | Satisfied | `just oracle::record --check-clean` regenerates pinned rows from Stim and `just oracle::run --milestone PF2` passes all implemented rows. |
| Benchmark contracts | Satisfied | Five report-only semantic rows include repeat-free generated-size, MPAD scale, repeat-count, repeat-body/state, and nonempty-flow sparse-index matrices with literal setup validation, exact work units, and feature-gated allocation bounds. |
| Clean committed allocation evidence | Satisfied | Four reports identify `HEAD=4f193f19cebf132f7baf0a3aa1cc799a153a71ed`, `local_modifications=false`, warmup, three runs, peak live allocation, and sampled resident delta. |
| GPT-5.6/max full-code-review closure | Satisfied | Successive core, evidence, benchmark, and resource confirmation passes report no remaining P0 through P2 blocker after all findings were fixed and rechecked. |

Milestone status: **Complete for the selected Rust transform scope**. All nineteen cases, review findings, clean evidence requirements, and audit criteria are closed. Python bindings, export products, broader feedback, heralded-record reversal, duplicate-target compatibility decisions, and behavior outside the finite ledger remain deferred or require a new exact plan.

## Next Work

Proceed to PFM-B5. Do not reopen PFM-B1 unless its frozen finite contract changes or a new exact-subcase plan deliberately expands the selected Rust transform scope.
