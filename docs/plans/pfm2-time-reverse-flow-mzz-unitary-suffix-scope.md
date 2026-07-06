# PFM2 Time-Reversed Flow MZZ Unitary Suffix Scope

## Summary

This slice promotes one measurement-rich `Circuit::time_reversed_for_flows` subcase from pinned Stim v1.16.0 `src/stim/util_top/circuit_inverse_qec.test.cc`: a single noiseless plain `MZZ` group followed by plain-qubit unitary instructions, including the upstream `flow_through_mzz_h_cx_s` packet.
It does not claim full multi-instruction measurement-rich time reversal, detector-flow QEC inverse parity, feedback, repeats, noise, or observable-aware rewrites.

## Owned Subcases

- Positive parity: `MZZ 0 1` followed by `H 0`, `CX 0 1`, and `S 1` reverses to `S_DAG 1`, `CX 0 1`, `H 0`, and `MZZ 0 1`, with the four upstream flows reversed exactly.
- Internal selector: one tag-free noiseless `MZZ` instruction with exactly one plain two-qubit group may be followed by tag-free argument-free unitary instructions whose targets are all plain qubits and whose inverse is accepted by the existing unitary inverse path.
- Validation: every input flow is checked against the original circuit with the existing sparse reverse-frame flow checker before the reversed circuit and flows are returned.
- Error behavior: sparse validation failures are wrapped with the selected-flow context so the rejected flow and selected measurement-rich surface are visible in the public Rust error, and observable terms are rejected because observable-aware rewrites remain outside this packet.

## Explicit Rejections

- Noisy `MZZ`, multi-record `MZZ`, tagged instructions, feedback targets, detector or observable annotations in the suffix, observable terms in requested flows, ordinary noise in the suffix, and repeat blocks remain unsupported for this selected path.
- Broader measurement-rich multi-instruction circuits, detector-flow rewrites with interleaved operations, feedback, repeats, noise, and observable-aware QEC inverse behavior remain active follow-up work under PFM2 and PFM5.
- Duplicate reset-only and duplicate measure-reset semantics remain governed by the existing spec-gap entries and are not touched by this slice.

## Comparator And Evidence

- Comparator class: exact structural parity against the pinned Stim v1.16.0 `flow_through_mzz_h_cx_s` expected circuit and expected flows.
- Core tests: `cargo test -p stab-core --test circuit_time_reverse_flow_mzz_suffix --quiet`.
- Oracle row: `pf2-time-reverse-flow-mzz-unitary-suffix-rust` runs the focused structural core test.
- Benchmark row: the existing non-primary report-only `pf2-time-reverse-flow-measurement` runner now includes the selected `MZZ` plus unitary-suffix packet and reports normalized `flows/s`.

## Done Criteria

- The exact upstream packet passes with expected circuit text and expected reversed flows.
- Unsupported nearby shapes fail closed before returning partial reversed output.
- The active PFM2, RPF2, checklist, oracle, test-porting, and benchmark metadata no longer describe the selected `MZZ` unitary suffix as unimplemented.
- Milestone-audit and full-code-review findings for this packet are fixed or logged as under-specification.
