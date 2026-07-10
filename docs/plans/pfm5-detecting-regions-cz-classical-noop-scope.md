# PF5 Detecting Regions `CZ` Classical-Only No-Op Scope

## Purpose

This PF5 slice promotes selected `CZ` groups whose two targets are classical bits in `circuit_detecting_regions`.
The owned behavior is detecting-region extraction only: `CZ rec[-k] sweep[j]`, `CZ sweep[j] rec[-k]`, and `CZ rec[-a] rec[-b]` are treated as no-ops for unsigned detector and logical-observable region propagation, matching pinned Stim v1.16.0 detector-slice behavior.
The already implemented `CZ sweep[i] sweep[j]` no-op remains part of the same classical-only `CZ` family.

## Comparator

The comparator is structural Rust API parity against pinned Stim v1.16.0 `stim diagram --type detslice-text` probes.
Pinned Stim prints only nonblank qubit rows, while Stab `FlexPauliString` output includes identity positions, so expected Stab strings such as `+_X` and `+__X` correspond to pinned nonblank `X` rows on qubits 1 and 2.

Representative probes:

```text
M 0
RX 1
TICK
CZ rec[-1] sweep[0]
MX 1
DETECTOR rec[-1]
```

```text
M 0 1
RX 2
TICK
CZ rec[-1] rec[-2]
MX 2
DETECTOR rec[-1]
```

Pinned Stim also accepts the detecting-region probe where the classical-only `CZ` references `rec[-1]` before any prior measurement record in forward circuit order.
Because this group has no qubit target and therefore cannot affect fixed unsigned detector sensitivity, Stab deliberately skips sparse-tracker record-offset validation for these `CZ` classical-only no-op groups inside detecting-region traversal.
This detecting-region slice did not itself broaden sampler, analyzer, detection-conversion, feedback-inlining, or CLI semantics.
PFM-B4 later promoted parser-accepted classical-only pairs as sign-only no-ops in flow generation and unsigned sparse tracking. Feedback inlining retains a narrower contract: record-only groups remain unsupported, sweep-only `CZ` groups are preserved unchanged like pinned Stim, and sweep-only `CX`, `CY`, `XCZ`, and `YCZ` groups are rejected.

## Owned Positive Scope

- `CZ rec[-k] sweep[j]` no-op detecting-region propagation.
- `CZ sweep[j] rec[-k]` no-op detecting-region propagation.
- `CZ rec[-a] rec[-b]` no-op detecting-region propagation.
- No record-history lookup for these classical-only `CZ` no-op groups.

## Owned Negative Scope

- Non-`CZ` record/sweep target groups remain fail-closed.
- Non-`CZ` record/record target groups remain fail-closed.
- Measurement-record feedback with one qubit target keeps the existing selected feedback placement rules.
- Sweep/qubit groups keep the existing selected gate-order-valid rules.
- Broader target shapes outside the promoted detecting-region set remain active PF5 work.

## Tests

- `cargo test -p stab-core --test detecting_regions_cz_classical_noop --quiet`
- `cargo test -p stab-core --test detecting_regions_cz_sweep_sweep --quiet`
- `cargo test -p stab-core detecting_regions_target_shape --quiet`
- `cargo test -p stab-core --test circuit_flows unsigned_stabilizer_flow_diagnostics_keep_unsupported_circuits_fail_closed --quiet`
- `cargo test -p stab-core circuit_with_inlined_feedback_keeps_cz_record_only_groups_unsupported --quiet`
- `cargo test -p stab-core circuit_with_inlined_feedback_rejects_non_cz_sweep_only_groups --quiet`

The new executable evidence lives in `crates/stab-core/tests/detecting_regions_cz_classical_noop.rs`.
The neighboring non-`CZ` record/sweep fail-closed evidence remains in `crates/stab-core/tests/detecting_regions_cz_sweep_sweep.rs`.
The flow-checker regressions now belong to the later PFM-B4 pairwise transition contract. The feedback-inlining regressions separately prove record-only rejection, `CZ` sweep-only preservation, and non-`CZ` sweep-only rejection at that transform boundary.

## Benchmarks

No benchmark row is added for this slice.
The implementation adds one narrow validation no-op branch and one detecting-region traversal splitter in a structural Rust utility path that already has no faithful pinned Stim CLI timing ratio.
Existing detecting-region benchmark rows remain report-only representative evidence for larger traversal workloads and must not be cited as direct timing evidence for this target-shape branch.

## Documentation And Metadata To Sync

- `docs/stab-feature-checklist.md`
- `docs/plans/non-deferred-partial-feature-milestones.md`
- `docs/plans/partial-feature-inventory.md`
- `docs/plans/rpf5-detecting-regions-progress-report.md`
- `docs/plans/rust-stim-drop-in-rewrite.md`
- `docs/plans/milestone-spec-gaps.md`
- `oracle/fixtures/manifest.csv`
