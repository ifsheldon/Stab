# PFM-B2 Gate-Surface Contract Groundwork Report

Date: 2026-07-10

Status: Complete historical groundwork. Final PFM-B2 semantic execution subsequently closed in `docs/plans/pfm-b2-gate-surface-progress-report.md`; references below to planned eighteen-case family rollups describe the pre-provenance-split state.

## Objective

Define one exhaustive, source-owned gate-by-surface contract before changing shared execution semantics.
This groundwork closes the planning ambiguity that previously allowed parser acceptance, one selected execution example, and full semantic support to be conflated.

## Scope

The contract is owned by canonical gate metadata in `crates/stab-core/src/gate.rs` and `crates/stab-core/src/gate/semantic_contract.rs`.
It remains crate-internal because execution-support status is a Stab implementation acceptance contract, not part of Stim's public `GateData` API; exposing it publicly would require a separate API-stability decision.
It covers all 81 canonical Stim v1.16.0 gates across these eight Stab surfaces:

- Circuit parser and validator.
- Measurement sampler.
- Deterministic reference sampler, including sweep-aware reference sampling used by detection conversion.
- Measurement-to-detection converter.
- Detector-frame execution.
- Public detection sampling.
- Circuit-to-DEM error analyzer.
- Stabilizer-flow generation.

Each canonical gate owns one of nineteen typed semantic families in the canonical gate table.
The contract derives twenty-two target-role patterns from the gate's canonical target rule, including required no-target instructions, empty lists accepted by target-bearing gates, plain and inverted measurement qubits, measurement pads, plain and measurement pairs, plain qubit-coordinate targets, measurement records, sweep bits, separately classified Hermitian and anti-Hermitian Pauli products, combiners, Pauli lists, detector or observable declarations, and all nine qubit, record, and sweep pair orderings accepted by the classical-control parser rule.

Every gate, surface, and declared target-pattern tuple has exactly one of these classifications:

- `execute`.
- `semantic_noop`.
- `annotation`.
- `lower_then_execute`.
- `unsupported_shape`.
- `not_applicable`.

There is no unknown, selected-example-only, or implicit fallback classification.
Decisions assume the gate's canonical argument rule, complete target grouping, in-range measurement records and sweep widths, and valid probabilities; malformed values and unavailable records remain separately owned negative-test contracts.

## Boundary Decisions

- Parser acceptance is recorded separately from execution behavior, so a syntactically accepted controlled-gate pair can still have a precise semantic rejection.
- `QUBIT_COORDS` accepts only plain qubit targets; inverted qubit targets are rejected at the parser boundary like pinned Stim v1.16.0.
- Empty target lists on target-bearing gates are explicit semantic no-ops except for annotation gates, which remain annotations, and correlated-error instructions, whose sampled branch state can suppress a following `ELSE_CORRELATED_ERROR`; required no-target instructions retain their annotation or structural behavior.
- `SPP` and `SPP_DAG` are `lower_then_execute` on semantic surfaces.
- Parser-accepted anti-Hermitian `MPP`, `SPP`, and `SPP_DAG` products have a typed `unsupported_shape` decision on semantic surfaces instead of sharing the executable Hermitian-product bucket.
- `I_ERROR` and `II_ERROR` are semantic no-ops.
- Non-heralded stochastic noise executes in sampling, detector-frame, detection-sampling, and analyzer surfaces, while reference sampling, detection conversion, and flow generation treat it as a semantic no-op.
- Heralded noise executes on every semantic surface because it owns measurement records even when deterministic reference behavior suppresses the sampled data error.
- `REPEAT` is parsed as control flow and is `not_applicable` as a regular instruction on execution surfaces; repeat-block execution and folding remain circuit-level contracts.
- Simulator, reference, conversion, frame, detection, and analyzer contracts preserve the directional classical-control rules: `CX` and `CY` accept classical control only in the first position, while `XCZ` and `YCZ` accept it only in the second position.
- `CZ` accepts either classical-control direction, and all-classical `CZ` pairs are semantic no-ops.
- Flow generation treats record/qubit feedback as executable and every other classical pair containing no two qubits as a semantic no-op in either orientation for `CX`, `CY`, `CZ`, `XCZ`, and `YCZ`, matching the existing flow-generator contract instead of inheriting simulator target order.
- Sweep-aware reference sampling and detection conversion execute supported sweep controls.
- The public measurement sampler has no sweep-input API, so supported sweep-control orderings use omitted all-false sweep semantics and are semantic no-ops; directionally invalid target shapes remain explicit rejections.
- Detector-frame execution, public detection sampling, the analyzer, and unsigned flow generation treat supported omitted-sweep controls as semantic no-ops under their selected all-false or sign-insensitive contracts.
- Outside flow generation, invalid quantum-to-classical directions and non-`CZ` all-classical pairs use typed invalid-combination exclusions.
- `FlowGenerator` is the sole PFM-B2 flow surface; flow checking, solving, and transform integration remain owned by the PFM-B4 shared flow-engine milestone.

## Tests

`crates/stab-core/src/gate/semantic_contract/tests.rs` provides ten focused groundwork checks:

- Every canonical gate, surface, and declared target pattern has exactly one decision, and all six behavior classes are represented.
- Every declared target pattern has a concrete representative accepted and mapped back to the same machine-readable pattern by the canonical parser rule and target classifier.
- Every `unsupported_shape` decision has a narrow typed exclusion, and metadata mismatches produce no decision so the completeness test fails instead of silently selecting a fallback.
- The directional `CX`, `XCZ`, and symmetric `CZ` classical-control matrix is frozen across reference sampling, detection conversion, analyzer, and no-sweep measurement sampling, while flow generation explicitly covers both record and sweep orientations.
- One mixed `CZ` target list classifies all nine qubit, record, and sweep pair roles in first-seen order while deduplicating a repeated role.
- Representative gates prove that semantic-family ownership lives in the canonical gate table instead of a test-only name list.
- Detector declarations, observable declarations, and `MPAD` constant roles remain distinct typed patterns instead of generic record or qubit lists.
- Empty correlated-error instructions execute on stochastic surfaces and remain semantic no-ops only on deterministic reference, conversion, and unsigned-flow surfaces; the sampler regression proves that an empty successful branch suppresses its following `ELSE_CORRELATED_ERROR`.
- Mixed Hermitian and anti-Hermitian Pauli-product groups classify independently, with parser acceptance separated from typed semantic rejection.
- Exhaustive one- through four-factor products over X, Y, and Z on two qubits cross-check the contract's Hermiticity classification against the independent compiled-sampler validation path.

These are contract-integrity tests, not final semantic parity evidence.
The eighteen `pfm3-contract-*` ledger cases covering all nineteen semantic families remain planned until each family has independently selectable exact, error-class, structural, semantic-invariant, state-equivalence, or statistical execution tests on every owned surface.
Ledger schema version 2 gives every case the exact eight-value `gate_surfaces` set and one or more typed `gate_families`; the validator rejects missing or duplicate surfaces and family declarations, rejects incomplete union coverage, and compares both wire-name sets against the canonical core contract through the feature-gated `ops-contracts` seam. Deterministic MPP, anti-Hermitian MPP rejection, deterministic MPAD, stochastic MPP, and stochastic MPAD are independent cases with exact upstream provenance and comparator classes; the stochastic MPAD record explicitly notes that pinned Stim has an implementation symbol but no direct stochastic MPAD GTest, so final closure requires a direct pinned-Stim statistical oracle.

## Analyzer Sweep Evidence Closure

The contract records the selected analyzer sweep boundary without adding speculative behavior.
The existing `pfm3-analyzer-sweep-matrix` ledger case and its primary and supporting oracle rows remain the complete selected evidence because pinned Stim v1.16.0 names only `ErrorAnalyzer.ignores_sweep_controls`, while Stab already covers that case plus the selected `CY`, `CZ`, `XCZ`, `YCZ`, and all-classical `CZ` matrix.
The maximum legal `sweep[16777215]` semantic regression is paired with a feature-gated allocation test and low-ID versus maximum-ID benchmark submeasurements; the allocation test permits at most two additional allocation calls and 1,024 additional total or peak-live bytes, ruling out state proportional to sweep-index magnitude.
The release-profile allocation probe recorded identical low-ID and maximum-ID measurements: 25 total allocation calls, 3,783 total bytes, 11 peak-live allocations, and 1,976 peak-live bytes for each submeasurement.
Future analyzer sweep shapes require a new failing pinned oracle, a newly selected public API, or an explicit compatibility-plan revision.

## Benchmarks

No benchmark row is added for the groundwork.
The new metadata is static, does not change an execution dispatch path, and is exercised only by contract generation and tests, so timing it would be a metadata microbenchmark without a user workload.
The existing `pf3-gate-semantic-wide` and `pf3-analyze-errors-sweep` report-only rows remain the representative evidence; the sweep row now reports the selected matrix plus separate low and maximum sweep-ID submeasurements with allocation metadata.
The ledger records no new mixed-contract row for static groundwork. Final PFM-B2 must add a mixed-contract row whenever a production compile or execution path begins consulting the contract; if dispatch remains unchanged, it must retain the no-new-row disposition and cite the static-only diff.

## Groundwork Acceptance Matrix

| Requirement | Status | Evidence |
| --- | --- | --- |
| Canonical metadata owns semantic families | Satisfied | `GateInfo::semantic_family` and compile-time explicit-family constructors for ambiguous categories |
| Every canonical gate and relevant surface is classified | Satisfied | 81 gates across all eight `GateSurface` values in `gate_surface_contract_covers_every_canonical_gate_surface_and_target_pattern` |
| Target roles are separate from gate names | Satisfied | Twenty-two typed `GateTargetPattern` values derived from canonical `TargetRule` metadata, including Hermitian and anti-Hermitian product shapes |
| No unknown or implicit fallback state exists | Satisfied | Exhaustive Rust enums and the completeness test |
| Unsupported shapes have narrow reasons | Satisfied | Three typed `GateShapeExclusion` variants and rejection-integrity tests |
| Analyzer sweep scope is evidence-closed | Satisfied for selected scope | `pfm3-analyzer-sweep-matrix`, existing core and CLI oracle rows, and the contract boundary above |
| Every semantic family has a ledger owner | Satisfied | Eighteen independently sourced cases cover the exact union of all nineteen `GateSemanticFamily` values, including identity-noise and control-flow shards |
| Ledger and core schemas cannot drift silently | Satisfied | The oracle validator compares all typed surface and family wire names against canonical core metadata on every check |
| Final family execution parity is proven | Pending | Eighteen planned `pfm3-contract-*` ledger cases retain planned status |
| Statistical false-positive budgets are validated | Pending | Required when the stochastic family shards are implemented |
| Final oracle shards and benchmark disposition are synchronized | Pending | Required during PFM-B2 final generated coverage |

## Verification

Completed for the groundwork:

- `cargo test -p stab-core gate_surface_contract --quiet`
- `cargo test -p stab-core correlated_error_branches_match_stim_else_semantics --quiet`
- `cargo test -p stab-core --test dem_analyzer_classical sweep --quiet`
- `cargo test -p stab-core --test gate_metadata --quiet`
- `cargo test -p stab-core --test stim_format parser_rejects_inverted_targets_except_result_gates_like_stim --quiet`
- `cargo test -p stab-oracle blocker_ledger --quiet`
- `cargo test -p stab-bench --features count-allocations pf3_analyzer_sweep_allocation_is_index_magnitude_independent --quiet`
- `just bench::compare-allocations --only pf3-analyze-errors-sweep --report target/benchmarks/pfm-b2-sweep-allocation-final`
- `cargo clippy -p stab-core --all-targets -- -D warnings`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --quiet`
- `just oracle::blockers --check-selectors`
- `just oracle::run --implemented-only`
- `just oracle::matrix --check`
- `just bench::smoke`
- `cargo fmt --all --check`

The final GPT-5.6/max milestone audit found no remaining technical or evidence defect after the provenance, parser, schema-linkage, mixed-control, and benchmark-trigger fixes; its sole final finding was the stale analyzer-sweep gap status and pending-closure sentence corrected in this documentation update.
The final GPT-5.6/max full code review reported no remaining P0 through P3 findings and approved the groundwork for its stated scope.

## Remaining PFM-B2 Work

- Generate independently selectable tests for all eighteen `pfm3-contract-*` cases covering the nineteen gate families.
- Implement every contract tuple that currently differs from production behavior, including the selected symmetric or reverse classical-control and sweep-aware reference or conversion shapes.
- Port deterministic exact or state-equivalence evidence for fixed-tableau, measurement, reset, pair-measurement, MPP, MPAD, SPP, annotation, and classical-control families.
- Run source-owned statistical comparisons for stochastic MPP, MPAD, Pauli noise, Pauli channels, depolarization, correlated errors, and heralded noise using the ledger shots, seeds, buckets, tolerance formula, and exact familywise-tail check.
- Add generated negative tests for every invalid target-role combination and cross-surface consistency tests wherever surfaces share semantics.
- Replace the broad oracle wording with family-owned rows and update ledger cases from planned only when their selectors and evidence are independently executable.
- Retain current report-only benchmarks while the contract remains static metadata; add a mixed-contract workload whenever production compile or execution dispatch begins consulting it.
- Run the final PFM-B2 milestone audit and GPT-5.6/max full code review after shared B3, B4, B1, and B5 foundations stabilize.
