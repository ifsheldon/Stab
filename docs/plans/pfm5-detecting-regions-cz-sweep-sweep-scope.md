# PFM5 Detecting Regions CZ Sweep-Sweep Scope

## Summary

This slice promotes the selected `CZ sweep[i] sweep[j]` bit-bit target shape in the Rust `circuit_detecting_regions` utility.
It is a narrow PF5 detector-utility target-shape promotion and does not broaden sweep-conditioned simulator parity, measurement-to-detection conversion, Python bindings, or public detector-sampler sweep APIs.

## Owned Subcases

- Accept `CZ` target groups made of exactly two sweep-bit targets.
- Treat accepted `CZ sweep[i] sweep[j]` groups as detecting-region no-ops because they cannot affect fixed detector or observable Pauli sensitivities without symbolic sweep inputs.
- Preserve existing support for `CZ sweep[k] q`, `CZ q sweep[k]`, and the other previously promoted one-sweep one-qubit controlled-Pauli no-op groups.
- Preserve detector, logical-observable, and tick filters.

## Explicit Rejections

- Keep non-`CZ` sweep/sweep controlled-Pauli groups rejected.
- Keep measurement-record/sweep groups rejected.
- Keep record/record feedback groups rejected.
- Keep invalid one-sweep one-qubit target orders rejected for `CX`, `CY`, `XCZ`, and `YCZ`.
- Keep non-selected sweep target shapes in other gates out of scope.

## Comparator And Evidence

The comparator class is structural Rust API parity against the selected detecting-region semantics already owned by Stab for sweep-only controlled-Pauli no-op traversal.
The sparse reverse tracker already treats target groups containing sweep bits and no measurement records as no-ops, while the detecting-region validator previously rejected this exact `CZ` bit-bit shape before traversal.
The new executable evidence must prove positive `CZ sweep/sweep` output and fail-closed non-`CZ` sweep/sweep plus record/sweep behavior through the `detecting_regions_target_shape` filter.

## Oracle And Benchmark Policy

- Oracle row: update the existing `pf5-detecting-regions-target-shapes-rust` row because the behavior belongs to the selected target-shape subset and is covered by the `detecting_regions_target_shape` test filter.
- Benchmark rows: no new row is added because this changes validation and an existing sparse-tracker no-op branch inside detecting-region traversal, not a new throughput-sensitive workload.
- Existing PF5 detecting-region benchmark rows remain report-only and must not be cited as direct timing evidence for this no-op branch.

## Verification

Targeted verification for this slice:

```sh
cargo test -p stab-core --test detecting_regions_cz_sweep_sweep --quiet
cargo test -p stab-core detecting_regions_target_shape --quiet
cargo test -p stab-oracle fixtures --quiet
just oracle::run --milestone PF5 --structural
just bench::smoke
```

Broader pre-commit verification follows the active `GOAL.md` work loop before commit.
