# PFM2 Selected MPAD Inverse-QEC Scope

## Summary

This note locks the selected `MPAD` record-tail slice for `Circuit::inverse_qec` and selected `Circuit::time_reversed_for_flows`.
It is a small QEC inverse parity packet derived from pinned Stim v1.16.0 `time_reversed_for_flows` probes and the upstream `CircuitFlowReverser::do_measuring_instruction` path.

## Implemented Subcases

- One top-level `MPAD` instruction with zero or more measurement-pad targets.
- Optional record-only `DETECTOR` declarations after that `MPAD`.
- Optional record-only `OBSERVABLE_INCLUDE` declarations after that `MPAD`, with distinct observable ids.
- Empty `DETECTOR` and empty `OBSERVABLE_INCLUDE` declarations are omitted from the inverse output after parity reduction, matching the pinned behavior.
- `MPAD` target order is reversed in the inverse output while probability arguments and tags are preserved.
- Detector and observable record references are remapped into the reversed measurement order, duplicate record references cancel by parity, and output record references are emitted in reversed measurement-index order.
- Empty-flow `Circuit::time_reversed_for_flows` uses the same selected inverse circuit and returns an empty inverted-flow list.
- Non-empty Pauli-only `Circuit::time_reversed_for_flows` batches through the selected MPAD record-tail packet are validated against the original circuit and returned with input and output Pauli endpoints swapped, matching the pinned Stim v1.16.0 identity-through-MPAD behavior.
- `InverseQecOptions { keep_measurements: true }` rejects the selected MPAD packet because that option remains selected only for the exact one-qubit reset-measure-detector packet.

## Explicit Rejections

- Record tails that reference measurements before the selected `MPAD` group.
- `OBSERVABLE_INCLUDE` declarations with Pauli targets.
- Duplicate `OBSERVABLE_INCLUDE` ids after the selected `MPAD`.
- Repeat blocks after the selected `MPAD`.
- Interleaved unitary, feedback, noise, measurement, reset, or other instructions after the selected `MPAD`.
- Measurement-record and observable terms in `Circuit::time_reversed_for_flows` batches through `MPAD`.
- Pauli-only flows that are not satisfied by the selected MPAD packet.

## Comparator And Evidence

Comparator class: structural Rust parity against pinned Stim v1.16.0 behavior observed through `stim.Circuit.time_reversed_for_flows(...)` plus upstream source inspection in `vendor/stim/src/stim/util_top/circuit_inverse_qec.cc`.

Pinned Stim v1.16.0 probes used for the promoted Pauli-only flow cases:

```text
input circuit:
MPAD 0
input flows:
X -> X
output circuit:
MPAD 0
output flows:
X -> X
---
input circuit:
MPAD 0 1
DETECTOR rec[-2]
OBSERVABLE_INCLUDE(0) rec[-1]
input flows:
X -> X
__Z -> __Z
output circuit:
MPAD 1 0
DETECTOR rec[-1]
OBSERVABLE_INCLUDE(0) rec[-2]
output flows:
X -> X
__Z -> __Z
```

Pinned Stim v1.16.0 also accepts `MPAD 0` with the flow `1 -> rec[-1]`, but that measurement-record flow behavior is intentionally not promoted in this slice because broader MPAD flow semantics still need exact tests, comparator rules, and resource policy.

Primary test evidence:

- `cargo test -p stab-core --test circuit_inverse_qec_mpad --quiet`

This test covers selected direct `inverse_qec` output, empty-flow and Pauli-only `time_reversed_for_flows`, measurement-record, observable, and unsatisfied-flow rejection, nearby fail-closed shapes, and `keep_measurements` rejection.

Oracle metadata:

- `pf2-inverse-qec-mpad-rust`

Benchmark policy:

- No separate benchmark row is required because this is a narrow selector and record-remapping semantic packet.
- It does not introduce a new high-volume traversal path beyond the existing QEC inverse and time-reversal dispatch.
- Broader measurement-rich `time_reversed_for_flows` performance remains represented by the existing report-only `pf2-time-reverse-flow-measurement` row.

## Remaining Scope

Broader MPAD flow semantics involving measurement-record or observable flow terms, Pauli observable tails, duplicate observable-id merging, repeats, feedback, and multi-instruction measurement-rich QEC inverse behavior remain under the existing PFM2 under-specification entry until a future exact-subcase plan names tests, comparator behavior, resource boundaries, oracle metadata, and benchmark policy.
