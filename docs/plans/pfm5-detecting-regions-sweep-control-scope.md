# PFM5 Detecting Regions Sweep-Control Scope

## Summary

This slice promotes selected gate-order-valid sweep-controlled Pauli target shapes in the Rust `circuit_detecting_regions` utility.
It is a narrow PFM5 detector-utility promotion, not broad sweep-conditioned simulator parity or Python binding work.
Later `CZ`-specific slices add selected `CZ sweep[i] sweep[j]`, `CZ rec[-k] sweep[j]`, `CZ sweep[j] rec[-k]`, and `CZ rec[-a] rec[-b]` bit-bit no-op cases without broadening the one-sweep one-qubit scope described here.

## Owned Subcases

- Accept `CX`, `CY`, `CZ`, `XCZ`, and `YCZ` target groups made of exactly one sweep-bit target and one plain qubit target, in the gate-specific target order accepted by pinned Stim v1.16.0.
- Treat the accepted groups as unsigned sign-only no-ops for detecting-region extraction, matching the existing `SparseReverseFrameTracker` behavior and pinned Stim v1.16.0 `Circuit.detecting_regions` output for selected examples.
- Support `CX sweep[k] q`, `CY sweep[k] q`, `CZ sweep[k] q`, `CZ q sweep[k]`, `XCZ q sweep[k]`, and `YCZ q sweep[k]`.
- Preserve existing detector, logical-observable, and tick filter behavior.
- Count produced measurement records through the circuit measurement-count API instead of detection-conversion planning, so detecting-region extraction does not inherit detection-conversion sweep-input rejection.

## Explicit Rejections

- Keep non-`CZ` sweep/sweep groups rejected where the sparse tracker cannot associate a plain qubit target with the operation.
- Keep non-`CZ` measurement-record/sweep groups rejected; selected `CZ` record/sweep groups are promoted later by `pfm5-detecting-regions-cz-classical-noop-scope.md`.
- Keep one-sweep one-qubit groups rejected when pinned Stim v1.16.0 rejects their target order.
- Keep controlled-Pauli groups with more than two targets rejected.
- Keep unsupported feedback positions and non-`CZ` record/record feedback groups rejected; selected `CZ` record/record groups are promoted later by `pfm5-detecting-regions-cz-classical-noop-scope.md`.
- Keep non-selected sweep target shapes in other gates out of scope.

## Comparator And Evidence

The comparator class is structural Rust API parity against pinned Stim v1.16.0 `Circuit.detecting_regions` for selected unsigned detecting-region outputs.
The source-owned upstream probe used `uv run --with stim==1.16.0 python` to verify that representative `CX`, `CY`, `CZ`, `XCZ`, and `YCZ` sweep/qubit circuits return the expected regions instead of raising.

## Oracle And Benchmark Policy

- Oracle row: update the existing `pf5-detecting-regions-target-shapes-rust` row because the behavior belongs to the selected target-shape subset and is covered by the `detecting_regions_target_shape` test filter.
- Benchmark rows: no new benchmark row is added because this changes validation and a sparse-tracker no-op branch inside the existing detecting-region traversal, not a new throughput-sensitive workload.
- Existing report-only detecting-region benchmark rows remain valid for repeat, target-filter, Clifford, and generated-code traversal workloads but are not cited as direct timing evidence for this no-op branch.

## Verification

Targeted verification for this slice:

```sh
cargo test -p stab-core detecting_regions_target_shape --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF5 --structural
```

Broader pre-commit verification follows the active `GOAL.md` work loop before commit.
