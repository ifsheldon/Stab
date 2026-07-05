# M9 Detector Utility Closure Plan

## Summary

This is a one-week follow-up plan for the detector-analysis utility work left behind by M9.
The goal is to promote three bounded surfaces from vague manifest-only or partial status into source-owned Rust evidence: simple detecting-region extraction, basic single-record missing-detector suggestions, and MPP feedback-inlining parity for `circuit_with_inlined_feedback`.
This plan deliberately avoided full transform API parity, exact loop refolding in that M9 slice, multi-record missing-detector row reduction, honeycomb and toric missing-detector analysis, full MPP stabilizer-product missing-detector analysis, sweep-aware `detect`, Python bindings, diagrams, and `explain_errors`.

Use `docs/plans/lessons-learned.md` as the guardrail.
The central rule for this plan is that upstream file names are not acceptance criteria.
Every implemented claim below names exact subcases, comparators, tests, benchmark rows, acceptance criteria, and deferrals.

## Starting State

`oracle/fixtures/manifest.csv` currently has `coverage-util-top-circuit-to-detecting-regions` as a manifest-only M9 row.
Its note names the simple H/CX/MXX detecting-region propagation case from `vendor/stim/src/stim/util_top/circuit_to_detecting_regions.test.cc`, but Stab does not yet expose a public Rust utility API for that behavior.

`oracle/fixtures/manifest.csv` currently has `coverage-util-top-missing-detectors` as a manifest-only M9 row.
Its note covers several unrelated subfamilies from `vendor/stim/src/stim/util_top/missing_detectors.test.cc`, including basic missing measurement detectors, repeated MPP stabilizer parity, observable interactions, honeycomb suffix cases, and toric-code global-stabilizer-product suffix cases.
This plan implements only the basic missing-measurement subset with single-record detector coverage and splits the broader cases into future manifest rows.

`oracle/fixtures/manifest.csv` currently marks `coverage-util-top-transform-without-feedback` as implemented for the M9-owned subset.
The implemented subset covers basic measurement-feedback removal, demolition feedback, interleaved-operation ordering, sweep-control preservation, and the public `m2d --ran_without_feedback` command case.
At the time of this plan, `docs/plans/milestone-spec-gaps.md` still recorded exact loop refolding and full MPP transform parity as gaps.
This plan closes the MPP feedback-transform gap only.
Broader repeat-contained feedback remains future work after the later PF2 selected bounded loop-refolding and nested bounded-repeat detector-parity slices.

## Public Rust Surface Changes

Add a detecting-regions API in `stab-core`:

```rust
pub struct DetectingRegionOptions {
    pub detectors: Vec<DemDetectorId>,
    pub ticks: Vec<u64>,
    pub ignore_anticommutation_errors: bool,
}

pub type DetectingRegionMap = BTreeMap<DemDetectorId, BTreeMap<u64, FlexPauliString>>;

pub fn circuit_detecting_regions(
    circuit: &Circuit,
    options: DetectingRegionOptions,
) -> CircuitResult<DetectingRegionMap>;
```

The API returns deterministic `BTreeMap` values ordered by detector id and tick.
The implemented tick contract must match the pinned Stim simple case: for the circuit `H 0; TICK; CX 0 1; TICK; MXX 0 1; DETECTOR rec[-1]`, detector `D0` at tick `0` is `X_` and detector `D0` at tick `1` is `XX`.
The API supports `ignore_anticommutation_errors = false`.
For the original M9 plan, `ignore_anticommutation_errors = true` returned a precise unsupported-domain error because broader gauge-handling was future scope.
PF5 later promoted ignored anticommutation mode, selected measurement-gauge ignored-mode behavior, and product-measurement gauge-cancellation behavior for the current supported detecting-region subset while leaving broader gauge behavior future work.

Add a basic missing-detectors API in `stab-core`:

```rust
pub struct MissingDetectorOptions {
    pub ignore_non_deterministic_measurements: bool,
}

pub fn missing_detectors(
    circuit: &Circuit,
    options: MissingDetectorOptions,
) -> CircuitResult<Circuit>;
```

The API returns a circuit containing only suggested missing `DETECTOR` instructions, matching Stim utility behavior for the owned basic cases with single-record detector coverage.
The API must not silently return an incomplete circuit for deferred stabilizer-product cases.
When it sees unsupported multi-record detector rows, MPP product analysis, observable-product analysis, honeycomb suffix analysis, or toric suffix analysis, it must return a precise unsupported-domain error until those subfamilies have their own plan.

Strengthen the existing `circuit_with_inlined_feedback(circuit: &Circuit) -> CircuitResult<Circuit>` API.
No new public transform function is required for MPP feedback inlining.
The implementation must preserve the existing `m2d --ran_without_feedback` behavior and add exact canonical output plus semantic parity for the upstream MPP transform subcase.

## Milestone 0: Split Rows And Add Red Tests

Goal: prevent this week from inheriting broad, ambiguous upstream-file acceptance.

Tasks:

- Split `coverage-util-top-circuit-to-detecting-regions` into one implemented target row for the simple H/CX/MXX case and one future row for multi-detector or broader gauge-handling expansion.
- Split `coverage-util-top-missing-detectors` into one implemented target row for basic reset, measure, duplicate-detector, nondeterministic-ignore, duplicate-target parity, and single-record partial multi-measurement cases, plus future rows for multi-record row reduction, repeated MPP stabilizer parity, observable interactions, honeycomb suffix, and toric suffix.
- Split the current `coverage-util-top-transform-without-feedback` note so it names the currently implemented subset, the new required MPP subcase, and the still-future exact loop-refolding subcase.
- Add or update red Rust tests before implementation work begins.
- Update `docs/plans/milestone-spec-gaps.md` only for gaps that remain after the row split, not for implementation defects that should be fixed in this plan.

Tests to add first:

```sh
cargo test -p stab-core detecting_regions_simple_h_cx_mxx --quiet
cargo test -p stab-core missing_detectors_basic --quiet
cargo test -p stab-core circuit_with_inlined_feedback_mpp --quiet
```

Acceptance:

- Every promoted oracle row has an executable comparator command instead of `manifest-only`.
- Every still-deferred row has a note naming exact deferred subcases, not just an upstream source file.
- Red tests fail for the missing implementation and pass only after the owned behavior is implemented.

## Milestone 1: Implement Simple Detecting Regions

Goal: implement the exact detecting-region utility subcase from pinned Stim v1.16.0 and expose it through a typed Rust API.

Implementation tasks:

- Add a `detecting_regions` module in `stab-core` and export the option type, map type, and function from `crates/stab-core/src/lib.rs`.
- Reuse the existing reverse-frame machinery where possible instead of building a duplicate Pauli propagation engine.
- Seed the reverse traversal from requested detector ids, propagate detector sensitivity backward through supported Clifford operations, and snapshot the requested tick layers.
- Support the operations required by the owned case: `H`, `CX`, `MXX`, `TICK`, and `DETECTOR rec[-1]`.
- Normalize returned `FlexPauliString` values to the circuit qubit width, using identity for qubits not in the region.
- Return a clear error when a requested detector id does not exist, when a requested tick is outside the circuit tick range, or when the circuit requires unsupported anticommutation or broader gauge handling.
- Return a clear unsupported-domain error for repeat blocks or gates outside the owned simple subset instead of attempting partial propagation.
- Preserve deterministic output ordering even when the caller provides duplicate or unsorted detector ids and ticks.

Required tests:

- Exact simple upstream case: `H 0; TICK; CX 0 1; TICK; MXX 0 1; DETECTOR rec[-1]` returns `D0 -> {0: X_, 1: XX}`.
- Duplicate detector and tick inputs are deduplicated in the returned map.
- Unknown detector id returns a domain error.
- Out-of-range tick returns a domain error.
- Unsupported broader gauge behavior and default false-mode anticommutation conflicts return domain errors.
- Unsupported gates and repeat blocks return domain errors that make the scoped subset explicit.

Oracle evidence:

- Update the promoted detecting-regions row to run the focused core test, using a command shape such as `cargo-test|-p|stab-core|detecting_regions_simple_h_cx_mxx`.

Acceptance:

- The promoted row passes under `just oracle::run --milestone M9`.
- The API is public from `stab-core` and uses typed detector ids, typed options, and `FlexPauliString`, not raw strings.
- The implementation does not claim repeat-block traversal, unsupported-gate propagation, broader multi-detector gauge handling, or full Python API parity.

## Milestone 2: Implement Basic Missing Detectors

Goal: implement a precise basic subset of Stim's `missing_detectors` utility without overclaiming the harder stabilizer-product families.

Owned upstream subcases:

- Empty circuit returns an empty suggestion circuit.
- `R 0; M 0; DETECTOR rec[-1]` returns empty because the measurement is already covered.
- Duplicate detector coverage for the same measurement still returns empty.
- `R 0; M 0` returns `DETECTOR rec[-1]`.
- `M 0` returns `DETECTOR rec[-1]` when `ignore_non_deterministic_measurements` is false.
- `M 0` returns empty when `ignore_non_deterministic_measurements` is true.
- `R 0 1; M 0 1; DETECTOR rec[-1]` returns `DETECTOR rec[-2]`.
- `M 0; DETECTOR rec[-1] rec[-1]` returns `DETECTOR rec[-1]` because duplicate references cancel within a detector row.

Implementation tasks:

- Add a `missing_detectors` module in `stab-core` and export the option type and function from `crates/stab-core/src/lib.rs`.
- Track absolute measurement record indices and map them to final relative `rec[-k]` references for output.
- Track simple reset-known qubit state for `R`, reset aliases, `M`, and measurement aliases needed by the owned cases.
- Track which absolute measurement records are already covered by existing `DETECTOR` declarations.
- Treat duplicate detector coverage as coverage, not as a reason to emit extra suggestions.
- Honor `ignore_non_deterministic_measurements` by treating initial Z-basis measurements as deterministic only in known-input mode and suppressing measurements whose basis is not determined.
- Canonically print the returned suggestion circuit using existing `Circuit::to_stim_string`.
- Reject unsupported multi-record detector rows, stabilizer-product cases, and observable-product cases with a precise error instead of returning a partial suggestion circuit.

Required tests:

- Exact canonical output tests for every owned upstream subcase listed above.
- A test proving suggested detectors use final relative record offsets, including the partial two-measurement case.
- A test proving duplicate detectors do not produce duplicate suggestions.
- A test proving duplicate targets within one detector row cancel instead of covering a measurement.
- A test proving the nondeterministic-ignore option changes `M 0` from one suggestion to empty.
- Negative tests for deferred multi-record detector row reduction, MPP stabilizer-product analysis, and observable-interaction analysis.

Oracle evidence:

- Replace the broad manifest-only missing-detectors row with an implemented basic row that runs the focused core test.
- Add future manifest-only rows for row reduction, MPP stabilizer parity, observable interactions, honeycomb suffix, and toric suffix, each with exact extraction notes.

Acceptance:

- The basic row passes under `just oracle::run --milestone M9`.
- Deferred missing-detector row reduction and broader families remain visible under `just oracle::list --milestone M9` or a future detector-analysis milestone.
- The implementation never claims full `missing_detectors.test.cc` parity.

## Milestone 3: Close MPP Feedback-Inlining Parity

Goal: finish the MPP feedback-transform subcase that is currently documented as a gap.

Owned upstream subcase:

- Port the `mpp` case from `vendor/stim/src/stim/util_top/transform_without_feedback.test.cc`.
- The transformed circuit must match Stim's canonical transformed circuit for the supported case.
- Detector error model semantics must match before and after transformation for the supported case.

Implementation tasks:

- Extend `circuit_with_inlined_feedback` so feedback controlled by MPP measurement records is represented in the sparse reverse frame tracker.
- Preserve non-feedback operation order.
- Preserve sweep-controlled operations as sweep-controlled operations.
- Preserve detector and observable meaning by rewriting affected `DETECTOR` and `OBSERVABLE_INCLUDE` declarations, not by merely deleting feedback instructions.
- Reject repeat blocks and unsupported classical controlled feedback gates with precise errors in this M9 slice until loop refolding and full feedback-gate parity are planned.
- Keep loop refolding out of scope for this M9 slice and keep the existing gap entry until a dedicated transform-refolding plan owns it.

Required tests:

- Exact canonical transformed circuit output for the upstream MPP case.
- DEM equivalence for the original and transformed MPP case, using existing analyzer or oracle helpers where available.
- A regression test proving existing `basic`, `demolition_feedback`, interleaved-ordering, sweep-preservation, and public `m2d --ran_without_feedback` tests still pass.
- Negative tests for unsupported MPP products, repeat blocks, and unsupported classical controlled feedback gates that remain out of the scoped transform.

Oracle evidence:

- Update `coverage-util-top-transform-without-feedback` metadata so the implemented subset includes MPP feedback transform parity.
- Keep a separate future note for loop refolding.

Acceptance:

- `cargo test -p stab-core circuit_with_inlined_feedback --quiet` passes with the new MPP test included.
- `just oracle::run --milestone M9` passes for implemented transform rows.
- `docs/plans/milestone-spec-gaps.md` no longer lists MPP transform parity as unresolved, but still lists loop refolding as future work for this historical M9 slice.
- Unsupported feedback-inlining shapes fail closed instead of passing through to a partial transform.

## Milestone 4: Add Report-Only Utility Benchmarks

Goal: create source-owned performance evidence for the new utility APIs without adding unstable rows to the 1.25x primary gate.

Benchmark rows:

| Row | Comparability | Workload | Measurement work | Gate policy |
| --- | --- | --- | --- | --- |
| `m9-detecting-regions-basic-batch` | `non-primary-report-only` | Run the simple H/CX/MXX detecting-regions case in a deterministic batch large enough to avoid tiny-timer noise. | `cases/s` and `regions/s`. | Report-only utility evidence, excluded from `--primary`. |
| `m9-missing-detectors-basic-batch` | `non-primary-report-only` | Run the owned basic missing-detectors corpus in a deterministic batch. | `cases/s` and `suggestions/s`. | Report-only utility evidence, excluded from `--primary`. |
| `m9-feedback-inline-mpp-batch` | `non-primary-report-only` | Transform the supported MPP feedback circuit repeatedly and black-box the output instruction count. | `transforms/s`. | Report-only utility evidence, excluded from `--primary`. |

Benchmark harness tasks:

- Add rows to `benchmarks/manifest.csv` with `non-primary-report-only` threshold classes and clear `report-only` compare notes.
- Add Stab-side runners in the existing M9 benchmark module or a new detector-utility benchmark module called from the M9 runner.
- Add `measurement_work` entries for every row.
- Extend benchmark manifest tests so every new row has a runner, measurement names, measurement work, and compare notes.
- Do not add these rows to `benchmarks/m12-primary-thresholds.json`.
- Do not add waivers unless a future primary gate selects these rows.

Benchmark commands:

```sh
just bench::smoke
just bench::baseline --only m9-detecting-regions-basic-batch --out target/benchmarks/m9-detecting-regions-basic-probe
just bench::compare --only m9-detecting-regions-basic-batch --baseline target/benchmarks/m9-detecting-regions-basic-probe/baseline.json --report target/benchmarks/m9-detecting-regions-basic-compare
just bench::baseline --only m9-missing-detectors-basic-batch --out target/benchmarks/m9-missing-detectors-basic-probe
just bench::compare --only m9-missing-detectors-basic-batch --baseline target/benchmarks/m9-missing-detectors-basic-probe/baseline.json --report target/benchmarks/m9-missing-detectors-basic-compare
just bench::baseline --only m9-feedback-inline-mpp-batch --out target/benchmarks/m9-feedback-inline-mpp-probe
just bench::compare --only m9-feedback-inline-mpp-batch --baseline target/benchmarks/m9-feedback-inline-mpp-probe/baseline.json --report target/benchmarks/m9-feedback-inline-mpp-compare
```

Acceptance:

- `just bench::smoke` validates the new rows.
- Focused baseline and compare probes produce machine-readable reports under `target/benchmarks/`.
- Documentation identifies these rows as report-only utility evidence and not beta-gate evidence.

## Milestone 5: Documentation, Audit, And Review Closure

Goal: make code, docs, oracle metadata, benchmark metadata, and feature status agree.

Documentation tasks:

- Update `docs/stab-feature-checklist.md` so detecting regions, basic missing detectors, and MPP feedback inlining are marked as implemented subsets with exact remaining gaps.
- Update `docs/plans/rust-stim-drop-in-rewrite.md` so the M9 follow-up text no longer says every detector-analysis utility row is manifest-only.
- Update `docs/plans/milestone-spec-gaps.md` to close the MPP transform gap and keep loop refolding, multi-record missing-detector row reduction, and broader missing-detector analysis as future gaps for this M9 slice.
- Add a completion report under `docs/plans/` after implementation, including test commands, oracle rows, benchmark rows, probe report paths, audit findings, review findings, and remaining exclusions.
- Keep `--detector_hypergraph` excluded and do not reopen any CLI detector-hypergraph support.

Audit and review tasks:

- Run milestone-audit against this plan after the implementation passes targeted tests.
- Fix implementation defects, missing-test findings, stale docs, stale manifest rows, and benchmark metadata problems found by milestone-audit.
- Log under-specified future scope in `docs/plans/milestone-spec-gaps.md` instead of trying to complete future work inside this plan.
- Run full-code-review against touched Rust core, oracle, benchmark, and docs surfaces.
- Fix full-code-review findings before claiming completion.

Acceptance:

- Checklist, roadmap, oracle manifest, benchmark manifest, completion report, and spec-gap log agree on implemented versus deferred behavior.
- Milestone-audit and full-code-review either have no actionable findings or every actionable finding is fixed.
- Under-specification findings are logged with exact future scope.

## One-Week Execution Schedule

Day 1:

- Complete Milestone 0.
- Add red tests and split oracle rows before implementation.
- Confirm the focused tests fail for missing behavior and that unchanged existing M9 tests still pass.

Day 2:

- Complete Milestone 1.
- Implement `circuit_detecting_regions`, export the API, and turn the detecting-regions row green.
- Run `cargo test -p stab-core detecting_regions --quiet` and `just oracle::run --milestone M9`.

Day 3:

- Complete Milestone 2.
- Implement `missing_detectors`, export the API, split future missing-detector rows, and turn the basic row green.
- Run `cargo test -p stab-core missing_detectors --quiet` and `just oracle::run --milestone M9`.

Day 4:

- Complete Milestone 3.
- Implement MPP feedback-transform parity and update the transform gap log.
- Run `cargo test -p stab-core circuit_with_inlined_feedback --quiet`, `cargo test -p stab-cli m2d --quiet`, and `just oracle::run --milestone M9`.

Day 5:

- Complete Milestones 4 and 5.
- Add report-only benchmark rows and focused probes.
- Synchronize docs, write the completion report, run milestone-audit, run full-code-review, fix findings, and run final verification.

## Required Targeted Test Commands

Run these during implementation as the relevant milestones land:

```sh
cargo test -p stab-core detecting_regions --quiet
cargo test -p stab-core missing_detectors --quiet
cargo test -p stab-core circuit_with_inlined_feedback --quiet
cargo test -p stab-cli m2d --quiet
cargo test -p stab-oracle fixtures --quiet
cargo test -p stab-bench m9 --quiet
just oracle::run --milestone M9
just oracle::run --implemented-only
just bench::smoke
```

Use narrower filters while iterating, but do not claim a milestone complete until the milestone-specific commands above pass.

## Required Final Verification

Before claiming this plan complete, run:

```sh
cargo fmt --all --check
cargo clippy -p stab-core -p stab-cli -p stab-oracle -p stab-bench --all-targets -- -D warnings
cargo test -p stab-core detecting_regions --quiet
cargo test -p stab-core missing_detectors --quiet
cargo test -p stab-core circuit_with_inlined_feedback --quiet
cargo test -p stab-cli m2d --quiet
cargo test -p stab-oracle fixtures --quiet
cargo test -p stab-bench m9 --quiet
just oracle::run --implemented-only
just bench::smoke
just maintenance::pre-commit
```

If the implementation touches shared parser, gate, sampler, analyzer, or benchmark infrastructure beyond the named utility modules, expand final verification to `cargo test --workspace --quiet`.

## Non-Goals And Stop Conditions

Do not implement exact loop refolding in this plan.
Do not implement multi-record missing-detector row reduction or full MPP stabilizer-product missing-detector analysis in this plan.
Do not implement honeycomb-code or toric-code missing-detector suffix analysis in this plan.
Do not implement sweep-aware `detect` sampling in this plan.
Do not implement Python, JS/WASM, diagrams, `explain_errors`, `repl`, QASM, Quirk, GPU, or new public graph/vector simulator APIs in this plan.
Do not add any new row to the primary 1.25x performance threshold file from a single report-only probe.
Stop and log a spec gap if an owned subcase requires a broader public API, full folded traversal, full detector-analyzer provenance, or a new simulator surface.
